use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde_json::Value;
use sqlx::postgres::PgPool;
use tracing::info;

use crate::error::AgentError;

static POOLS: LazyLock<Mutex<HashMap<String, PgPool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn execute(uri: &str, query: &str) -> Result<Value, AgentError> {
    let pool = {
        let pools = POOLS.lock().unwrap();
        pools.get(uri).cloned()
    };

    let pool = match pool {
        Some(pool) => pool,
        None => {
            info!("creating new postgres connection pool");
            let pool = PgPool::connect(uri).await?;
            POOLS.lock().unwrap().insert(uri.to_string(), pool.clone());
            pool
        }
    };

    let json_query = format!("SELECT JSON_AGG(t) FROM ({}) t", query);
    let result: Option<Value> = sqlx::query_scalar(&json_query).fetch_one(&pool).await?;
    Ok(result.unwrap_or(Value::Array(vec![])))
}
