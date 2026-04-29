use serde_json::Value;
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::info;

use crate::error::AgentError;
use crate::params::ConnectionParams;

pub async fn execute(conn: &ConnectionParams, query: &str) -> Result<Value, AgentError> {
    let ConnectionParams::Mssql {
        host,
        port,
        instance,
        username,
        password,
        database,
        trust_cert,
    } = conn
    else {
        return Err(AgentError::Connection(
            "mssql executor received non-mssql connection params".into(),
        ));
    };

    info!(host = %host, port = port, "connecting to mssql");

    let mut config = Config::new();
    config.host(host);
    config.port(*port);
    if *trust_cert {
        config.trust_cert();
    }
    if let Some(name) = instance {
        config.instance_name(name);
    }
    if let Some(db) = database {
        config.database(db);
    }
    config.authentication(AuthMethod::sql_server(username, password));

    let addr = format!("{host}:{port}");
    let tcp = TcpStream::connect(&addr)
        .await
        .map_err(|e| AgentError::Connection(format!("mssql TCP error: {e}")))?;
    tcp.set_nodelay(true)
        .map_err(|e| AgentError::Connection(format!("mssql TCP error: {e}")))?;

    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .map_err(|e| AgentError::Connection(format!("mssql connection error: {e}")))?;

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
