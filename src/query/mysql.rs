use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde_json::Value;
use sqlx::mysql::{MySqlPool, MySqlRow};
use sqlx::{Column, Row, TypeInfo};
use tracing::info;

use crate::error::AgentError;

static POOLS: LazyLock<Mutex<HashMap<String, MySqlPool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn execute(uri: &str, query: &str) -> Result<Value, AgentError> {
    let pool = {
        let pools = POOLS.lock().unwrap();
        pools.get(uri).cloned()
    };

    let pool = match pool {
        Some(pool) => pool,
        None => {
            info!("creating new mysql connection pool");
            let pool = MySqlPool::connect(uri).await?;
            POOLS.lock().unwrap().insert(uri.to_string(), pool.clone());
            pool
        }
    };

    let rows: Vec<MySqlRow> = sqlx::query(query).fetch_all(&pool).await?;

    let result: Vec<Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (i, col) in row.columns().iter().enumerate() {
                let val = row_value_to_json(row, i, col.type_info().name());
                obj.insert(col.name().to_string(), val);
            }
            Value::Object(obj)
        })
        .collect();

    Ok(Value::Array(result))
}

fn row_value_to_json(row: &MySqlRow, index: usize, type_name: &str) -> Value {
    if type_name == "BOOLEAN" {
        return row
            .try_get::<Option<bool>, _>(index)
            .ok()
            .flatten()
            .map_or(Value::Null, Value::from);
    }
    if type_name.contains("INT") {
        if type_name.contains("UNSIGNED") {
            return row
                .try_get::<Option<u64>, _>(index)
                .ok()
                .flatten()
                .map_or(Value::Null, Value::from);
        }
        return row
            .try_get::<Option<i64>, _>(index)
            .ok()
            .flatten()
            .map_or(Value::Null, Value::from);
    }
    if type_name == "FLOAT" || type_name == "DOUBLE" {
        return row
            .try_get::<Option<f64>, _>(index)
            .ok()
            .flatten()
            .map_or(Value::Null, Value::from);
    }
    if type_name == "JSON" {
        return row
            .try_get::<Option<String>, _>(index)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
    }
    // DECIMAL, VARCHAR, TEXT, DATE, DATETIME, TIMESTAMP, etc.
    row.try_get::<Option<String>, _>(index)
        .ok()
        .flatten()
        .map_or(Value::Null, Value::from)
}
