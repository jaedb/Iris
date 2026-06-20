// SPDX-License-Identifier: MPL-2.0

use anyhow::Error;
use async_tungstenite::tungstenite::{Message as WsMessage, Utf8Bytes};
use futures::channel::mpsc;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task;
use tracing::{debug, error, info, instrument, trace, warn};

struct Peer {
    receive_task_handle: task::JoinHandle<()>,
    send_task_handle: task::JoinHandle<Result<(), Error>>,
    sender: mpsc::Sender<String>,
}

struct State {
    tx: Option<mpsc::Sender<(String, Option<Utf8Bytes>)>>,
    peers: HashMap<String, Peer>,
}

#[derive(Clone)]
pub struct Server {
    state: Arc<Mutex<State>>,
}

#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error("error during handshake {0}")]
    Handshake(#[from] async_tungstenite::tungstenite::Error),
    #[error("error during TLS handshake {0}")]
    TLSHandshake(#[from] std::io::Error),
    #[error("timeout during TLS handshake {0}")]
    TLSHandshakeTimeout(#[from] tokio::time::error::Elapsed),
}

impl Server {
    #[instrument(level = "debug", skip(factory))]
    pub fn spawn<
        I: for<'a> Deserialize<'a>,
        O: Serialize + std::fmt::Debug + Send + Sync,
        Factory: FnOnce(Pin<Box<dyn Stream<Item = (String, Option<I>)> + Send>>) -> St,
        St: Stream<Item = (String, O)> + Send + Unpin + 'static,
    >(
        factory: Factory,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<(String, Option<Utf8Bytes>)>(1000);
        let mut handler = factory(Box::pin(rx.filter_map(|(peer_id, msg)| async move {
            if let Some(msg) = msg {
                match serde_json::from_str::<I>(&msg) {
                    Ok(msg) => Some((peer_id, Some(msg))),
                    Err(err) => {
                        warn!("Failed to parse incoming message: {} ({})", err, msg);
                        None
                    }
                }
            } else {
                Some((peer_id, None))
            }
        })));

        let state = Arc::new(Mutex::new(State {
            tx: Some(tx),
            peers: HashMap::new(),
        }));

        let state_clone = state.clone();
        task::spawn(async move {
            while let Some((peer_id, msg)) = handler.next().await {
                match serde_json::to_string(&msg) {
                    Ok(msg_str) => {
                        let sender = {
                            let mut state = state_clone.lock().unwrap();
                            if let Some(peer) = state.peers.get_mut(&peer_id) {
                                Some(peer.sender.clone())
                            } else {
                                None
                            }
                        };

                        if let Some(mut sender) = sender {
                            trace!("Sending {}", msg_str);
                            let _ = sender.send(msg_str).await;
                        }
                    }
                    Err(err) => {
                        warn!("Failed to serialize outgoing message: {}", err);
                    }
                }
            }
        });

        Self { state }
    }

    #[instrument(level = "debug", skip(state))]
    fn remove_peer(state: Arc<Mutex<State>>, peer_id: &str) {
        if let Some(mut peer) = state.lock().unwrap().peers.remove(peer_id) {
            let peer_id = peer_id.to_string();
            task::spawn(async move {
                peer.sender.close_channel();
                if let Err(err) = peer.send_task_handle.await {
                    trace!(peer_id = %peer_id, "Error while joining send task: {}", err);
                }

                if let Err(err) = peer.receive_task_handle.await {
                    trace!(peer_id = %peer_id, "Error while joining receive task: {}", err);
                }
            });
        }
    }

    #[instrument(level = "debug", skip(self, stream))]
    pub async fn accept_async<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        &mut self,
        stream: S,
    ) -> Result<String, ServerError> {
        let ws = match async_tungstenite::tokio::accept_async(stream).await {
            Ok(ws) => ws,
            Err(err) => {
                warn!("Error during the websocket handshake: {}", err);
                return Err(ServerError::Handshake(err));
            }
        };

        let this_id = uuid::Uuid::new_v4().to_string();
        info!(this_id = %this_id, "New WebSocket connection");

        // 1000 is completely arbitrary, we simply don't want infinite piling
        // up of messages as with unbounded
        let (websocket_sender, mut websocket_receiver) = mpsc::channel::<String>(1000);

        let this_id_clone = this_id.clone();
        let (mut ws_sink, mut ws_stream) = ws.split();
        let send_task_handle = task::spawn(async move {
            let mut res = Ok(());
            loop {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    websocket_receiver.next(),
                )
                .await
                {
                    Ok(Some(msg)) => {
                        trace!(this_id = %this_id_clone, "sending {}", msg);
                        res = ws_sink.send(WsMessage::text(msg)).await;
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(_) => {
                        trace!(this_id = %this_id_clone, "timeout, sending ping");
                        res = ws_sink.send(WsMessage::Ping(Default::default())).await;
                    }
                }

                if let Err(ref err) = res {
                    error!(this_id = %this_id_clone, "Quitting send loop: {err}");
                    break;
                }
            }

            debug!(this_id = %this_id_clone, "Done sending");

            let _ = ws_sink.close(None).await;

            res.map_err(Into::into)
        });

        let mut tx = self.state.lock().unwrap().tx.clone();
        let this_id_clone = this_id.clone();
        let state_clone = self.state.clone();
        let receive_task_handle = task::spawn(async move {
            if let Some(tx) = tx.as_mut() {
                if let Err(err) = tx
                    .send((
                        this_id_clone.clone(),
                        Some(
                            serde_json::json!({
                                "type": "newPeer",
                            })
                            .to_string()
                            .into(),
                        ),
                    ))
                    .await
                {
                    warn!(this = %this_id_clone, "Error handling message: {:?}", err);
                }
            }
            while let Some(msg) = ws_stream.next().await {
                info!("Received message {msg:?}");
                match msg {
                    Ok(WsMessage::Text(msg)) => {
                        if let Some(tx) = tx.as_mut() {
                            if let Err(err) = tx.send((this_id_clone.clone(), Some(msg))).await {
                                warn!(this = %this_id_clone, "Error handling message: {:?}", err);
                            }
                        }
                    }
                    Ok(WsMessage::Close(reason)) => {
                        info!(this_id = %this_id_clone, "connection closed: {:?}", reason);
                        break;
                    }
                    Ok(WsMessage::Pong(_)) => {
                        continue;
                    }
                    Ok(_) => warn!(this_id = %this_id_clone, "Unsupported message type"),
                    Err(err) => {
                        warn!(this_id = %this_id_clone, "recv error: {}", err);
                        break;
                    }
                }
            }

            if let Some(tx) = tx.as_mut() {
                let _ = tx.send((this_id_clone.clone(), None)).await;
            }

            Self::remove_peer(state_clone, &this_id_clone);
        });

        self.state.lock().unwrap().peers.insert(
            this_id.clone(),
            Peer {
                receive_task_handle,
                send_task_handle,
                sender: websocket_sender,
            },
        );

        Ok(this_id)
    }
}
