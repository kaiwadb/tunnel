use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use mongodb::{Client, Collection, bson::Document};
use serde_json::{Value, to_value};
use tracing::info;

use crate::error::AgentError;

static CLIENTS: LazyLock<Mutex<HashMap<String, Client>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn execute(
    uri: &str,
    collection: String,
    pipeline: Vec<Document>,
) -> Result<Value, AgentError> {
    let client = {
        let clients = CLIENTS.lock().unwrap();
        clients.get(uri).cloned()
    };

    let client = match client {
        Some(client) => client,
        None => {
            info!(
                host = uri.split('@').next_back().unwrap_or("unknown"),
                "creating new mongodb client"
            );
            let client = Client::with_uri_str(uri).await?;
            CLIENTS.lock().unwrap().insert(uri.to_string(), client.clone());
            client
        }
    };

    let db = client
        .default_database()
        .ok_or(AgentError::Connection("no default database specified in URI".into()))?;
    let collection: Collection<Document> = db.collection(&collection);

    let mut cursor = collection.aggregate(pipeline).await?;
    let mut batch = Vec::new();

    while cursor.advance().await? {
        let doc = cursor.deserialize_current()?;
        batch.push(doc);
    }

    Ok(to_value(&batch)?)
}
