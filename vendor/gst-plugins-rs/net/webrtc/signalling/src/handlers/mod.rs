// SPDX-License-Identifier: MPL-2.0

use anyhow::{anyhow, Error};
use anyhow::{bail, Context};
use futures::prelude::*;
use futures::ready;
use gst_plugin_webrtc_protocol as p;
use p::PeerStatus;
use pin_project_lite::pin_project;
use std::collections::{HashMap, HashSet, VecDeque};
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tracing::log::error;
use tracing::{info, instrument, warn};

type PeerId = String;

#[derive(Clone)]
struct Session {
    id: String,
    producer: PeerId,
    consumer: PeerId,
}

impl Session {
    fn other_peer_id(&self, id: &str) -> Result<&str, Error> {
        if self.producer == id {
            Ok(&self.consumer)
        } else if self.consumer == id {
            Ok(&self.producer)
        } else {
            bail!("Peer {id} is not part of {}", self.id)
        }
    }
}

pin_project! {
    #[must_use = "streams do nothing unless polled"]
    pub struct Handler {
        #[pin]
        stream: Pin<Box<dyn Stream<Item=(String, Option<p::IncomingMessage>)> + Send>>,
        items: VecDeque<(String, p::OutgoingMessage)>,
        peers: HashMap<PeerId, PeerStatus>,
        sessions: HashMap<String, Session>,
        consumer_sessions: HashMap<String, HashSet<String>>,
        producer_sessions: HashMap<String, HashSet<String>>,
    }
}

impl Handler {
    #[instrument(level = "debug", skip(stream))]
    /// Create a handler
    pub fn new(
        stream: Pin<Box<dyn Stream<Item = (String, Option<p::IncomingMessage>)> + Send>>,
    ) -> Self {
        Self {
            stream,
            items: VecDeque::new(),
            peers: Default::default(),
            sessions: Default::default(),
            consumer_sessions: Default::default(),
            producer_sessions: Default::default(),
        }
    }

    #[instrument(level = "trace", skip(self))]
    fn handle(
        mut self: Pin<&mut Self>,
        peer_id: &str,
        msg: p::IncomingMessage,
    ) -> Result<(), Error> {
        match msg {
            p::IncomingMessage::NewPeer => {
                self.peers.insert(peer_id.to_string(), Default::default());
                self.items.push_back((
                    peer_id.into(),
                    p::OutgoingMessage::Welcome {
                        peer_id: peer_id.to_string(),
                    },
                ));

                Ok(())
            }
            p::IncomingMessage::SetPeerStatus(status) => self.set_peer_status(peer_id, &status),
            p::IncomingMessage::StartSession(message) => {
                self.start_session(peer_id, &message.peer_id, message.offer.as_deref())
            }
            p::IncomingMessage::Peer(peermsg) => self.handle_peer_message(peer_id, peermsg),
            p::IncomingMessage::List => self.list_producers(peer_id),
            p::IncomingMessage::ListConsumers => self.list_consumers(peer_id),
            p::IncomingMessage::EndSession(msg) => self.end_session(peer_id, &msg.session_id),
        }
    }

    fn handle_peer_message(&mut self, peer_id: &str, peermsg: p::PeerMessage) -> Result<(), Error> {
        let session_id = &peermsg.session_id;

        let Some(session) = self.sessions.get(session_id) else {
            warn!(
                peer_id,
                "Received peer message for unknown session {session_id}"
            );
            return Ok(());
        };

        if matches!(
            peermsg.peer_message,
            p::PeerMessageInner::Sdp(p::SdpMessage::Offer { .. })
        ) && peer_id == session.consumer
        {
            bail!(
                r#"cannot forward offer from "{peer_id}" to "{}" as "{peer_id}" is not the producer"#,
                session.producer,
            );
        }

        self.items.push_back((
            session.other_peer_id(peer_id)?.to_owned(),
            p::OutgoingMessage::Peer(p::PeerMessage {
                session_id: session_id.to_string(),
                peer_message: peermsg.peer_message.clone(),
            }),
        ));

        Ok(())
    }

    fn stop_producer(&mut self, peer_id: &str) {
        if let Some(session_ids) = self.producer_sessions.remove(peer_id) {
            for session_id in session_ids {
                if let Err(e) = self.end_session(peer_id, &session_id) {
                    error!("Could not end session {session_id}: {e:?}");
                }
            }
        }
    }

    fn stop_consumer(&mut self, peer_id: &str) {
        if let Some(session_ids) = self.consumer_sessions.remove(peer_id) {
            for session_id in session_ids {
                if let Err(e) = self.end_session(peer_id, &session_id) {
                    error!("Could not end session {session_id}: {e:?}");
                }
            }
        }
    }

    #[instrument(level = "debug", skip(self))]
    /// Remove a peer, this can cause sessions to be ended
    fn remove_peer(&mut self, peer_id: &str) {
        info!(peer_id = %peer_id, "removing peer");
        let peer_status = match self.peers.remove(peer_id) {
            Some(peer_status) => peer_status,
            _ => return,
        };

        self.stop_producer(peer_id);
        self.stop_consumer(peer_id);

        for (id, p) in self.peers.iter() {
            if !p.listening() {
                continue;
            }

            let message = p::OutgoingMessage::PeerStatusChanged(PeerStatus {
                roles: Default::default(),
                meta: peer_status.meta.clone(),
                peer_id: Some(peer_id.to_string()),
            });
            self.items.push_back((id.to_string(), message));
        }
    }

    #[instrument(level = "debug", skip(self))]
    /// End a session between two peers
    fn end_session(&mut self, peer_id: &str, session_id: &str) -> Result<(), Error> {
        let Some(session) = self.sessions.remove(session_id) else {
            warn!(
                peer_id,
                "Received end session message for unknown session {session_id}"
            );
            return Ok(());
        };

        self.consumer_sessions
            .entry(session.consumer.clone())
            .and_modify(|sessions| {
                sessions.remove(session_id);
            });

        self.producer_sessions
            .entry(session.producer.clone())
            .and_modify(|sessions| {
                sessions.remove(session_id);
            });

        self.items.push_back((
            session.other_peer_id(peer_id)?.to_string(),
            p::OutgoingMessage::EndSession(p::EndSessionMessage {
                session_id: session_id.to_string(),
            }),
        ));

        Ok(())
    }

    fn filter_peers<F>(&self, filter: F) -> Vec<p::Peer>
    where
        F: Fn(&PeerStatus) -> bool,
    {
        self.peers
            .iter()
            .filter_map(move |(peer_id, peer)| {
                filter(peer).then_some(p::Peer {
                    id: peer_id.clone(),
                    meta: peer.meta.clone(),
                })
            })
            .collect()
    }

    /// List producer peers
    #[instrument(level = "debug", skip(self))]
    fn list_producers(&mut self, peer_id: &str) -> Result<(), Error> {
        self.items.push_back((
            peer_id.to_string(),
            p::OutgoingMessage::List {
                producers: self.filter_peers(PeerStatus::producing),
            },
        ));

        Ok(())
    }

    /// List consumer peers
    #[instrument(level = "debug", skip(self))]
    fn list_consumers(&mut self, peer_id: &str) -> Result<(), Error> {
        self.items.push_back((
            peer_id.to_string(),
            p::OutgoingMessage::ListConsumers {
                consumers: self.filter_peers(PeerStatus::consuming),
            },
        ));

        Ok(())
    }

    /// Register peer as a producer
    #[instrument(level = "debug", skip(self))]
    fn set_peer_status(&mut self, peer_id: &str, status: &p::PeerStatus) -> Result<(), Error> {
        let old_status = self
            .peers
            .get(peer_id)
            .context(anyhow!("Peer '{peer_id}' hasn't been welcomed"))?;

        if status == old_status {
            info!("Status for '{}' hasn't changed", peer_id);

            return Ok(());
        }

        if status.producing() && status.consuming() {
            bail!(
                "Cannot register peer {} as both producer and passive consumer",
                peer_id
            );
        }

        if old_status.producing() && !status.producing() {
            self.stop_producer(peer_id);
        } else if old_status.consuming() && !status.consuming() {
            self.stop_consumer(peer_id);
        }

        let mut status = status.clone();
        status.peer_id = Some(peer_id.to_string());
        self.peers.insert(peer_id.to_string(), status.clone());
        for (id, peer) in &self.peers {
            if !peer.listening() {
                continue;
            }

            self.items.push_back((
                id.to_string(),
                p::OutgoingMessage::PeerStatusChanged(p::PeerStatus {
                    peer_id: Some(peer_id.to_string()),
                    roles: status.roles.clone(),
                    meta: status.meta.clone(),
                }),
            ));
        }

        info!(peer_id = %peer_id, "registered as {:?}", status.roles);

        Ok(())
    }

    /// Start a session between two peers
    #[instrument(level = "debug", skip(self))]
    fn start_session(
        &mut self,
        from_id: &str,
        to_id: &str,
        offer: Option<&str>,
    ) -> Result<(), Error> {
        let from = self
            .peers
            .get(from_id)
            .ok_or_else(|| anyhow!("Peer with ID '{}' not found", from_id))?;
        let to = self
            .peers
            .get(to_id)
            .ok_or_else(|| anyhow!("Peer with ID '{}' not found", to_id))?;

        let (producer_id, consumer_id) = if to.producing() {
            (to_id, from_id)
        } else if to.consuming() {
            (from_id, to_id)
        } else {
            bail!(
                "Missing a producer or a consumer: id {} roles {:?}, id {} roles {:?}",
                from_id,
                from.roles,
                to_id,
                to.roles
            );
        };

        let session_id = uuid::Uuid::new_v4().to_string();
        self.sessions.insert(
            session_id.clone(),
            Session {
                id: session_id.clone(),
                consumer: consumer_id.to_string(),
                producer: producer_id.to_string(),
            },
        );
        self.consumer_sessions
            .entry(consumer_id.to_string())
            .or_default()
            .insert(session_id.clone());
        self.producer_sessions
            .entry(producer_id.to_string())
            .or_default()
            .insert(session_id.clone());
        self.items.push_back((
            consumer_id.to_string(),
            p::OutgoingMessage::SessionStarted {
                peer_id: producer_id.to_string(),
                session_id: session_id.clone(),
            },
        ));
        self.items.push_back((
            producer_id.to_string(),
            p::OutgoingMessage::StartSession {
                peer_id: consumer_id.to_string(),
                session_id: session_id.clone(),
                offer: offer.map(String::from),
            },
        ));

        info!(id = %session_id, producer_id = %producer_id, consumer_id = %consumer_id, "started a session");

        Ok(())
    }
}

impl Stream for Handler {
    type Item = (String, p::OutgoingMessage);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let this = self.as_mut().project();

            if let Some(item) = this.items.pop_front() {
                break Poll::Ready(Some(item));
            }

            match ready!(this.stream.poll_next(cx)) {
                Some((peer_id, msg)) => {
                    if let Some(msg) = msg {
                        if let Err(err) = self.as_mut().handle(&peer_id, msg) {
                            self.items.push_back((
                                peer_id.to_string(),
                                p::OutgoingMessage::Error {
                                    details: err.to_string(),
                                },
                            ));
                        }
                    } else {
                        self.remove_peer(&peer_id);
                    }
                }
                None => {
                    break Poll::Ready(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::channel::mpsc;
    use serde_json::json;

    async fn new_peer(
        tx: &mut mpsc::UnboundedSender<(String, Option<p::IncomingMessage>)>,
        handler: &mut Handler,
        peer_id: &str,
    ) {
        tx.send((peer_id.to_string(), Some(p::IncomingMessage::NewPeer)))
            .await
            .unwrap();

        let res = handler.next().await.unwrap();
        assert_eq!(
            res,
            (
                peer_id.to_string(),
                p::OutgoingMessage::Welcome {
                    peer_id: peer_id.to_string()
                }
            )
        );
    }

    #[tokio::test]
    async fn test_register_producer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        tx.send((
            "producer".to_string(),
            Some(p::IncomingMessage::SetPeerStatus(p::PeerStatus {
                roles: vec![p::PeerRole::Producer],
                meta: None,
                peer_id: None,
            })),
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_list_producers() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            meta: Some(json!({"display-name":"foobar".to_string()})),
            roles: vec![p::PeerRole::Producer],
            peer_id: None,
        });

        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        let message = p::IncomingMessage::List;
        tx.send(("listener".to_string(), Some(message)))
            .await
            .unwrap();

        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "listener");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::List {
                producers: vec![p::Peer {
                    id: "producer".to_string(),
                    meta: Some(json!(
                        {"display-name": "foobar".to_string()
                    })),
                }],
            }
        );
    }

    #[tokio::test]
    async fn test_welcome() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "consumer").await;
    }

    #[tokio::test]
    async fn test_listener() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;
        new_peer(&mut tx, &mut handler, "listener").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Listener],
            meta: None,
            peer_id: None,
        });
        tx.send(("listener".to_string(), Some(message)))
            .await
            .unwrap();

        let _ = handler.next().await.unwrap();

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: Some(json!({
                "display-name": "foobar".to_string(),
            })),
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "listener");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::PeerStatusChanged(p::PeerStatus {
                roles: vec![p::PeerRole::Producer],
                peer_id: Some("producer".to_string()),
                meta: Some(json!({
                        "display-name": Some("foobar".to_string()),
                    }
                ))
            })
        );
    }

    #[tokio::test]
    async fn test_start_session() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();

        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing {sent_message:?}"),
        };

        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "producer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::StartSession {
                peer_id: "consumer".to_string(),
                session_id: session_id.to_string(),
                offer: None,
            }
        );
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        assert_eq!(
            handler.next().await.unwrap(),
            (
                "producer".into(),
                p::OutgoingMessage::StartSession {
                    peer_id: "consumer".into(),
                    session_id: session_id.clone(),
                    offer: None
                }
            )
        );

        new_peer(&mut tx, &mut handler, "listener").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Listener],
            meta: None,
            peer_id: None,
        });
        tx.send(("listener".to_string(), Some(message)))
            .await
            .unwrap();
        let _ = handler.next().await.unwrap();

        handler.remove_peer("producer");
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "consumer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::EndSession(p::EndSessionMessage { session_id })
        );

        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "listener");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::PeerStatusChanged(PeerStatus {
                roles: vec![],
                peer_id: Some("producer".to_string()),
                meta: Default::default()
            })
        );
    }

    #[tokio::test]
    async fn test_end_session_consumer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let _ = handler.next().await.unwrap();

        let message = p::IncomingMessage::EndSession(p::EndSessionMessage {
            session_id: session_id.clone(),
        });

        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "producer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::EndSession(p::EndSessionMessage { session_id })
        );
    }

    #[tokio::test]
    async fn test_disconnect_consumer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let _ = handler.next().await.unwrap();

        tx.send(("consumer".to_string(), None)).await.unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "producer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::EndSession(p::EndSessionMessage { session_id })
        );
    }

    #[tokio::test]
    async fn test_end_session_producer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let _ = handler.next().await.unwrap();

        let message = p::IncomingMessage::EndSession(p::EndSessionMessage {
            session_id: session_id.clone(),
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "consumer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::EndSession(p::EndSessionMessage { session_id })
        );
    }

    #[tokio::test]
    async fn test_sdp_exchange() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let _ = handler.next().await.unwrap();

        let message = p::IncomingMessage::Peer(p::PeerMessage {
            session_id: session_id.clone(),
            peer_message: p::PeerMessageInner::Sdp(p::SdpMessage::Offer {
                sdp: "offer".to_string(),
            }),
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "consumer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::Peer(p::PeerMessage {
                session_id: session_id.clone(),
                peer_message: p::PeerMessageInner::Sdp(p::SdpMessage::Offer {
                    sdp: "offer".to_string()
                })
            })
        );
    }

    #[tokio::test]
    async fn test_ice_exchange() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let _ = handler.next().await.unwrap();

        let message = p::IncomingMessage::Peer(p::PeerMessage {
            session_id: session_id.clone(),
            peer_message: p::PeerMessageInner::Ice {
                candidate: "candidate".to_string(),
                sdp_m_line_index: 42,
            },
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "consumer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::Peer(p::PeerMessage {
                session_id: session_id.clone(),
                peer_message: p::PeerMessageInner::Ice {
                    candidate: "candidate".to_string(),
                    sdp_m_line_index: 42
                }
            })
        );

        let message = p::IncomingMessage::Peer(p::PeerMessage {
            session_id: session_id.clone(),
            peer_message: p::PeerMessageInner::Ice {
                candidate: "candidate".to_string(),
                sdp_m_line_index: 42,
            },
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "producer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::Peer(p::PeerMessage {
                session_id: session_id.clone(),
                peer_message: p::PeerMessageInner::Ice {
                    candidate: "candidate".to_string(),
                    sdp_m_line_index: 42
                }
            })
        );
    }

    #[tokio::test]
    async fn test_sdp_exchange_wrong_direction_offer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let _ = handler.next().await.unwrap();

        let message = p::IncomingMessage::Peer(p::PeerMessage {
            session_id,
            peer_message: p::PeerMessageInner::Sdp(p::SdpMessage::Offer {
                sdp: "offer".to_string(),
            }),
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let response = handler.next().await.unwrap();

        assert_eq!(response,
            (
                "consumer".into(),
                p::OutgoingMessage::Error {
                    details: r#"cannot forward offer from "consumer" to "producer" as "consumer" is not the producer"#.into()
                }
            )
        );
    }

    #[tokio::test]
    async fn test_start_session_no_producer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "consumer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::Error {
                details: "Peer with ID 'producer' not found".into()
            }
        );
    }

    #[tokio::test]
    async fn test_stop_producing() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "producer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::StartSession {
                peer_id: "consumer".to_string(),
                session_id: session_id.clone(),
                offer: None,
            }
        );

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "consumer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::EndSession(p::EndSessionMessage {
                session_id: session_id.clone(),
            })
        );
    }

    #[tokio::test]
    async fn test_unregistering_with_listeners() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "listener").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Listener],
            meta: None,
            peer_id: None,
        });
        tx.send(("listener".to_string(), Some(message)))
            .await
            .unwrap();
        let _ = handler.next().await.unwrap();

        new_peer(&mut tx, &mut handler, "producer").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "listener");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::PeerStatusChanged(PeerStatus {
                roles: vec![p::PeerRole::Producer],
                peer_id: Some("producer".to_string()),
                meta: Default::default()
            })
        );

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();

        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing {sent_message:?}"),
        };

        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "producer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::StartSession {
                peer_id: "consumer".to_string(),
                session_id: session_id.clone(),
                offer: None,
            }
        );

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        assert_eq!(
            handler.next().await.unwrap(),
            (
                "consumer".into(),
                p::OutgoingMessage::EndSession(p::EndSessionMessage {
                    session_id: session_id.clone(),
                })
            )
        );

        assert_eq!(
            handler.next().await.unwrap(),
            (
                "listener".into(),
                p::OutgoingMessage::PeerStatusChanged(PeerStatus {
                    roles: vec![],
                    peer_id: Some("producer".to_string()),
                    meta: Default::default()
                })
            )
        );
    }

    #[tokio::test]
    async fn test_start_session_no_consumer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();

        assert_eq!(peer_id, "consumer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::Error {
                details: "Peer with ID 'consumer' not found".into()
            }
        );
    }

    #[tokio::test]
    async fn test_start_session_twice() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: Some(json!( {"display-name": "foobar".to_string() })),
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "consumer").await;

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session0_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let _ = handler.next().await.unwrap();

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });

        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session1_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        assert_ne!(session0_id, session1_id);
    }

    #[tokio::test]
    async fn test_start_session_stop_producing() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "producer").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer, p::PeerRole::Listener],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();
        let _ = handler.next().await.unwrap();

        new_peer(&mut tx, &mut handler, "producer-consumer").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            ..Default::default()
        });
        tx.send(("producer-consumer".to_string(), Some(message)))
            .await
            .unwrap();
        handler.next().await.unwrap();

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "producer".to_string(),
            offer: None,
        });
        tx.send(("producer-consumer".to_string(), Some(message)))
            .await
            .unwrap();
        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "producer-consumer");
        let session0_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Listener],
            ..Default::default()
        });
        tx.send(("producer-consumer".to_string(), Some(message)))
            .await
            .unwrap();
        handler.next().await.unwrap();

        let message = p::IncomingMessage::List;
        tx.send(("producer-consumer".to_string(), Some(message)))
            .await
            .unwrap();

        handler.next().await.unwrap();
        handler
            .sessions
            .get(&session0_id)
            .expect("Session should remain");
    }

    #[tokio::test]
    async fn test_start_session_passive_consumer() {
        let (mut tx, rx) = mpsc::unbounded();
        let mut handler = Handler::new(Box::pin(rx));

        new_peer(&mut tx, &mut handler, "consumer").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Consumer],
            meta: None,
            peer_id: None,
        });
        tx.send(("consumer".to_string(), Some(message)))
            .await
            .unwrap();

        new_peer(&mut tx, &mut handler, "producer").await;
        let message = p::IncomingMessage::SetPeerStatus(p::PeerStatus {
            roles: vec![p::PeerRole::Producer],
            meta: None,
            peer_id: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        let message = p::IncomingMessage::StartSession(p::StartSessionMessage {
            peer_id: "consumer".to_string(),
            offer: None,
        });
        tx.send(("producer".to_string(), Some(message)))
            .await
            .unwrap();

        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "consumer");
        let session_id = match sent_message {
            p::OutgoingMessage::SessionStarted {
                ref peer_id,
                ref session_id,
            } => {
                assert_eq!(peer_id, "producer");
                session_id.to_string()
            }
            _ => panic!("SessionStarted message missing"),
        };

        let (peer_id, sent_message) = handler.next().await.unwrap();
        assert_eq!(peer_id, "producer");
        assert_eq!(
            sent_message,
            p::OutgoingMessage::StartSession {
                peer_id: "consumer".to_string(),
                session_id,
                offer: None,
            }
        );
    }
}
