use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use tracing::info;

use crate::error::AgentError;
use crate::params::ConnectionParams;

static POOLS: LazyLock<Mutex<HashMap<String, PgPool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn execute(conn: &ConnectionParams, query: &str) -> Result<Value, AgentError> {
    let ConnectionParams::Postgres {
        host,
        port,
        username,
        password,
        database,
        sslmode,
    } = conn
    else {
        return Err(AgentError::Connection(
            "postgres executor received non-postgres connection params".into(),
        ));
    };

    let cache_key = format!("{host}:{port}/{database}@{username}");
    let pool = {
        let pools = POOLS.lock().unwrap();
        pools.get(&cache_key).cloned()
    };

    let pool = match pool {
        Some(pool) => pool,
        None => {
            info!(host = %host, port = port, database = %database, "creating new postgres connection pool");
            let options = PgConnectOptions::new()
                .host(host)
                .port(*port)
                .username(username)
                .password(password)
                .database(database)
                .ssl_mode(sslmode.to_sqlx());
            let pool = PgPoolOptions::new().connect_with(options).await?;
            POOLS.lock().unwrap().insert(cache_key, pool.clone());
            pool
        }
    };

    let json_query = format!("SELECT JSON_AGG(t) FROM ({}) t", query);
    let result: Option<Value> = sqlx::query_scalar(&json_query).fetch_one(&pool).await?;
    Ok(result.unwrap_or(Value::Array(vec![])))
}
