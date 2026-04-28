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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Query {
    pub uri: String,
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

        let result = match self.engine {
            Engine::Mongo { .. } => {
                let MongoData {
                    collection,
                    pipeline,
                } = from_value(self.data)?;
                info!(collection = %collection, "executing mongodb query");
                debug!(pipeline = ?pipeline);
                mongo::execute(&self.uri, collection, pipeline).await
            }
            Engine::Postgres { .. } => {
                let query: String = from_value(self.data)?;
                info!("executing postgresql query");
                debug!(query = %query);
                postgres::execute(&self.uri, &query).await
            }
            Engine::MySQL { .. } => {
                let query: String = from_value(self.data)?;
                info!("executing mysql query");
                debug!(query = %query);
                mysql::execute(&self.uri, &query).await
            }
            Engine::MsSql { .. } => {
                let query: String = from_value(self.data)?;
                info!("executing mssql query");
                debug!(query = %query);
                mssql::execute(&self.uri, &query).await
            }
            Engine::Clickhouse { .. } => {
                let query: String = from_value(self.data)?;
                info!("executing clickhouse query");
                debug!(query = %query);
                clickhouse::execute(&self.uri, &query).await
            }
            ref engine => {
                return Err(AgentError::UnsupportedEngine(format!("{engine:?}")));
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
