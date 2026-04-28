#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("MongoDB error: {0}")]
    Mongo(#[from] mongodb::error::Error),

    #[error("SQL error: {0}")]
    Sql(#[from] sqlx::Error),

    #[error("Unsupported engine: {0}")]
    UnsupportedEngine(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Invalid header: {0}")]
    InvalidHeader(#[from] tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue),
}

impl From<tokio_tungstenite::tungstenite::Error> for AgentError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        AgentError::WebSocket(Box::new(err))
    }
}
