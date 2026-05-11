use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::time::interval;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::Request},
};
use tracing::{debug, error, info};

use crate::communication::{TunnelResult, TunnelResultMsg, PayloadFromTunnel, ServerCommand, ServerCommandMsg};
use crate::error::TunnelError;

const HEARTBEAT_PERIOD: Duration = Duration::from_secs(15);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

pub async fn run(uri: String, token: String) -> Result<(), TunnelError> {
    let mut request = uri.into_client_request().map_err(|e| {
        TunnelError::Connection(format!("invalid WebSocket URI: {e}"))
    })?;
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {}", token).parse()?);

    loop {
        match connect_and_handle(request.clone()).await {
            Ok(()) => {
                info!("connection ended normally");
                break;
            }
            Err(e) => {
                error!(error = %e, "connection error");
                info!(delay_secs = RECONNECT_DELAY.as_secs(), "reconnecting");
                tokio::time::sleep(RECONNECT_DELAY).await;
            }
        }
    }

    Ok(())
}

async fn connect_and_handle(request: Request<()>) -> Result<(), TunnelError> {
    let (ws_stream, _) = connect_async(request).await?;
    info!("connected");

    let (mut write, mut read) = ws_stream.split();
    let mut heartbeat_interval = interval(HEARTBEAT_PERIOD);

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_server_message(text.to_string(), &mut write).await {
                            error!(error = %e, "error handling message");
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        write.send(Message::Pong(payload)).await?;
                    }
                    Some(Ok(Message::Pong(_))) => continue,
                    Some(Ok(Message::Close(frame))) => {
                        info!(?frame, "connection closed by server");
                        break;
                    }
                    Some(Ok(Message::Binary(_) | Message::Frame(_))) => continue,
                    Some(Err(e)) => return Err(e.into()),
                    None => return Err(TunnelError::Connection("connection closed unexpectedly".into())),
                }
            }
            _ = heartbeat_interval.tick() => {
                write.send(Message::Ping("heartbeat".into())).await?;
            }
        }
    }

    Ok(())
}

async fn handle_server_message(
    text: String,
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
) -> Result<(), TunnelError> {
    let msg: ServerCommandMsg = serde_json::from_str(&text)?;

    match msg.payload {
        ServerCommand::Query(query) => {
            let data = query.execute().await?;
            debug!("query completed, sending response");

            let response = PayloadFromTunnel::Result(TunnelResultMsg {
                channel: msg.channel,
                payload: TunnelResult::QueryResult(data),
            });
            let response_txt = serde_json::to_string(&response)?;
            write
                .send(Message::Text(response_txt.as_str().into()))
                .await?;
        }
    }

    Ok(())
}
