use serde_json::Value;
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::info;
use url::Url;

use crate::error::AgentError;

pub async fn execute(uri: &str, query: &str) -> Result<Value, AgentError> {
    let (config, addr) = parse_uri(uri)?;

    info!("connecting to mssql");

    let tcp = TcpStream::connect(&addr)
        .await
        .map_err(|e| AgentError::Connection(format!("mssql TCP error: {e}")))?;
    tcp.set_nodelay(true)
        .map_err(|e| AgentError::Connection(format!("mssql TCP error: {e}")))?;

    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| AgentError::Connection(format!("mssql connection error: {e}")))?;

    // Use FOR JSON PATH for server-side JSON conversion
    let json_query = format!(
        "SELECT * FROM ({query}) AS q FOR JSON PATH, INCLUDE_NULL_VALUES"
    );

    let stream = client
        .query(&json_query, &[])
        .await
        .map_err(|e| AgentError::Connection(format!("mssql query error: {e}")))?;

    let rows = stream
        .into_first_result()
        .await
        .map_err(|e| AgentError::Connection(format!("mssql fetch error: {e}")))?;

    // FOR JSON PATH splits long JSON across multiple rows — concatenate them
    let json_string: String = rows
        .iter()
        .filter_map(|row| row.try_get::<&str, _>(0).ok().flatten())
        .collect();

    if json_string.is_empty() {
        return Ok(Value::Array(vec![]));
    }

    let result: Value = serde_json::from_str(&json_string)?;
    Ok(result)
}

fn parse_uri(uri: &str) -> Result<(Config, String), AgentError> {
    // Normalize mssql:// or sqlserver:// scheme to http:// for URL parsing
    let normalized = if uri.starts_with("mssql://") {
        uri.replacen("mssql://", "http://", 1)
    } else if uri.starts_with("sqlserver://") {
        uri.replacen("sqlserver://", "http://", 1)
    } else {
        uri.to_string()
    };

    let parsed = Url::parse(&normalized)
        .map_err(|e| AgentError::Connection(format!("invalid mssql URI: {e}")))?;

    let host = parsed.host_str().unwrap_or("localhost");
    let port = parsed.port().unwrap_or(1433);

    let mut config = Config::new();
    config.host(host);
    config.port(port);
    config.trust_cert();

    if let Some(db) = parsed
        .path()
        .strip_prefix('/')
        .filter(|s| !s.is_empty())
    {
        config.database(db);
    }

    let username = if parsed.username().is_empty() {
        "sa"
    } else {
        parsed.username()
    };
    let password = parsed.password().unwrap_or("");
    config.authentication(AuthMethod::sql_server(username, password));

    let addr = format!("{host}:{port}");
    Ok((config, addr))
}
