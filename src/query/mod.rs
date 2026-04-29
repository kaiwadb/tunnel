mod clickhouse;
mod mongo;
mod mssql;
mod mysql;
mod postgres;

use mongodb::bson::Document;
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_value};
use std::time::Instant;
use tracing::{debug, error, info};

use crate::engine::Engine;
use crate::error::AgentError;
use crate::params::ConnectionParams;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Query {
    pub connection: ConnectionParams,
    /// Engine version metadata. Kept on the wire for future version-conditional
    /// logic; not used for connection (backend validates engine/connection
    /// variants match before forwarding).
    pub engine: Engine,
    pub data: Value,
}

#[derive(Deserialize)]
struct MongoData {
    collection: String,
    pipeline: Vec<Document>,
}

impl Query {
    pub async fn execute(self) -> Result<Value, AgentError> {
        let start = Instant::now();

        let Query { connection, engine: _, data } = self;

        let result = match &connection {
            ConnectionParams::Mongo { .. } => {
                let MongoData {
                    collection,
                    pipeline,
                } = from_value(data)?;
                info!(collection = %collection, "executing mongodb query");
                debug!(pipeline = ?pipeline);
                mongo::execute(&connection, collection, pipeline).await
            }
            ConnectionParams::Postgres { .. } => {
                let query: String = from_value(data)?;
                info!("executing postgresql query");
                debug!(query = %query);
                postgres::execute(&connection, &query).await
            }
            ConnectionParams::Mysql { .. } => {
                let query: String = from_value(data)?;
                info!("executing mysql query");
                debug!(query = %query);
                mysql::execute(&connection, &query).await
            }
            ConnectionParams::Mssql { .. } => {
                let query: String = from_value(data)?;
                info!("executing mssql query");
                debug!(query = %query);
                mssql::execute(&connection, &query).await
            }
            ConnectionParams::Clickhouse { .. } => {
                let query: String = from_value(data)?;
                info!("executing clickhouse query");
                debug!(query = %query);
                clickhouse::execute(&connection, &query).await
            }
        };

        let duration = start.elapsed();

        match &result {
            Ok(value) => {
                info!(duration_ms = duration.as_millis(), "query completed");
                debug!(result = %serde_json::to_string_pretty(value).unwrap_or_default());
            }
            Err(e) => {
                error!(duration_ms = duration.as_millis(), error = %e, "query failed");
            }
        }

        result
    }
}
