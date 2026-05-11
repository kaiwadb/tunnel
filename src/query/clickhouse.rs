use std::sync::LazyLock;

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tracing::info;

use crate::error::TunnelError;
use crate::params::ConnectionParams;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .build()
        .expect("failed to build HTTP client")
});

pub async fn execute(conn: &ConnectionParams, query: &str) -> Result<Value, TunnelError> {
    let ConnectionParams::Clickhouse {
        host,
        port,
        username,
        password,
        database,
        secure,
    } = conn
    else {
        return Err(TunnelError::Connection(
            "clickhouse executor received non-clickhouse connection params".into(),
        ));
    };

    let scheme = if *secure { "https" } else { "http" };
    let url = format!("{scheme}://{host}:{port}/");

    info!(url = %url, "executing clickhouse query");

    let mut request = HTTP_CLIENT.post(&url);

    if let Some(db) = database {
        request = request.query(&[("database", db.as_str())]);
    }

    if let Some(user) = username {
        request = request.basic_auth(user, password.as_deref());
    }

    let body = format!("{query} FORMAT JSON");
    let response = request
        .body(body)
        .send()
        .await
        .map_err(|e| TunnelError::Connection(format!("clickhouse HTTP error: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(TunnelError::Connection(format!(
            "clickhouse returned {status}: {text}"
        )));
    }

    let result: ClickhouseResponse = response
        .json()
        .await
        .map_err(|e| TunnelError::Connection(format!("clickhouse response parse error: {e}")))?;

    Ok(Value::Array(result.data))
}

#[derive(Deserialize)]
struct ClickhouseResponse {
    data: Vec<Value>,
}
