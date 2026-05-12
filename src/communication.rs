use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::query::Query;

/// Server -> tunnel frame. The id is the originating
/// `tunnel_commands.id` row in the server's database, used to correlate
/// responses without any in-process routing table.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Command { id: i64, request: ServerCommand },
}

/// Tunnel -> server frame. The id always echoes back the originating
/// command's id.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelMessage {
    Result { id: i64, payload: TunnelResult },
    Error { id: i64, error: String },
}

#[derive(Debug, Deserialize)]
pub enum ServerCommand {
    Query(Query),
}

#[derive(Debug, Serialize)]
pub enum TunnelResult {
    QueryResult(Value),
}
