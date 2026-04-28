use std::sync::LazyLock;

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tracing::info;
use percent_encoding::percent_decode_str;
use url::Url;

use crate::error::AgentError;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .build()
        .expect("failed to build HTTP client")
});

pub async fn execute(uri: &str, query: &str) -> Result<Value, AgentError> {
    let parsed = parse_uri(uri)?;

    info!(host = %parsed.url, "executing clickhouse query");

    let mut request = HTTP_CLIENT.post(&parsed.url);

    if let Some(db) = &parsed.database {
        request = request.query(&[("database", db.as_str())]);
    }

    if let Some(user) = &parsed.username {
        request = request.basic_auth(user, parsed.password.as_deref());
    }

    let body = format!("{query} FORMAT JSON");
    let response = request
        .body(body)
        .send()
        .await
        .map_err(|e| AgentError::Connection(format!("clickhouse HTTP error: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AgentError::Connection(format!(
            "clickhouse returned {status}: {text}"
        )));
    }

    let result: ClickhouseResponse = response
        .json()
        .await
        .map_err(|e| AgentError::Connection(format!("clickhouse response parse error: {e}")))?;

    Ok(Value::Array(result.data))
}

#[derive(Deserialize)]
struct ClickhouseResponse {
    data: Vec<Value>,
}

struct ParsedUri {
    url: String,
    database: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

fn parse_uri(uri: &str) -> Result<ParsedUri, AgentError> {
    // Normalize clickhouse:// scheme to http:// for parsing
    let normalized = if uri.starts_with("clickhouse://") {
        uri.replacen("clickhouse://", "http://", 1)
    } else {
        uri.to_string()
    };

    let parsed = Url::parse(&normalized)
        .map_err(|e| AgentError::Connection(format!("invalid clickhouse URI: {e}")))?;

    let host = parsed.host_str().unwrap_or("localhost");
    let port = parsed.port().unwrap_or(8123);
    let database = parsed
        .path()
        .strip_prefix('/')
        .filter(|s| !s.is_empty())
        .map(String::from);
    let username = if parsed.username().is_empty() {
        None
    } else {
        Some(
            percent_decode_str(parsed.username())
                .decode_utf8_lossy()
                .into_owned(),
        )
    };
    let password = parsed.password().map(|p| {
        percent_decode_str(p)
            .decode_utf8_lossy()
            .into_owned()
    });

    Ok(ParsedUri {
        url: format!("http://{host}:{port}/"),
        database,
        username,
        password,
    })
}
