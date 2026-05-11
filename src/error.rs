#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("MongoDB error: {0}")]
    Mongo(#[from] mongodb::error::Error),

    #[error("SQL error: {0}")]
    Sql(#[from] sqlx::Error),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Invalid header: {0}")]
    InvalidHeader(#[from] tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue),
}

impl From<tokio_tungstenite::tungstenite::Error> for TunnelError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        TunnelError::WebSocket(Box::new(err))
    }
}
