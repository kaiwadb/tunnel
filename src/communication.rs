use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::query::Query;

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerCommand {
    Query(Query),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TunnelResult {
    QueryResult(Value),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCommandMsg {
    pub channel: String,
    pub payload: ServerCommand,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TunnelResultMsg {
    pub channel: String,
    pub payload: TunnelResult,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TunnelNotification {}

#[derive(Debug, Serialize, Deserialize)]
pub enum PayloadFromTunnel {
    Result(TunnelResultMsg),
    Notification(TunnelNotification),
}
