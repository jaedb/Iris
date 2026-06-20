// SPDX-License-Identifier: MPL-2.0

/// The default protocol used by the signalling server
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
/// Messages sent from the server to peers
pub enum OutgoingMessage {
    /// Welcoming message, sets the Peer ID linked to a new connection
    #[serde(rename_all = "camelCase")]
    Welcome { peer_id: String },
    /// Notifies listeners that a peer status has changed
    PeerStatusChanged(PeerStatus),
    /// Instructs a peer to generate an offer or an answer and inform about the session ID
    #[serde(rename_all = "camelCase")]
    StartSession {
        peer_id: String,
        session_id: String,
        offer: Option<String>,
    },
    /// Let consumer know that the requested session is starting with the specified identifier
    #[serde(rename_all = "camelCase")]
    SessionStarted { peer_id: String, session_id: String },
    /// Signals that the session the peer was in was ended
    EndSession(EndSessionMessage),
    /// Messages directly forwarded from one peer to another
    Peer(PeerMessage),
    /// Provides the current list of producers
    #[serde(rename_all = "camelCase")]
    List { producers: Vec<Peer> },
    /// Provides the current list of consumers (awaiting a session request)
    #[serde(rename_all = "camelCase")]
    ListConsumers { consumers: Vec<Peer> },
    /// Notifies that an error occurred with the peer's current session
    #[serde(rename_all = "camelCase")]
    Error { details: String },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
/// Register with a peer type
pub enum PeerRole {
    /// Register as a producer
    Producer,
    /// Register as a listener
    Listener,
    /// Register as a passive consumer (awaiting a session initiated by a producer peer)
    Consumer,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PeerStatus {
    pub roles: Vec<PeerRole>,
    pub meta: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub peer_id: Option<String>,
}

impl PeerStatus {
    pub fn producing(&self) -> bool {
        self.roles.iter().any(|t| matches!(t, PeerRole::Producer))
    }

    pub fn listening(&self) -> bool {
        self.roles.iter().any(|t| matches!(t, PeerRole::Listener))
    }

    pub fn consuming(&self) -> bool {
        self.roles.iter().any(|t| matches!(t, PeerRole::Consumer))
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// Ask the server to start a session (either as a producer or a consumer)
pub struct StartSessionMessage {
    /// Identifies the peer
    pub peer_id: String,
    /// An offer if the consumer peer wants the producer to answer
    pub offer: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
/// Conveys a SDP
pub enum SdpMessage {
    /// Conveys an offer
    #[serde(rename_all = "camelCase")]
    Offer {
        /// The SDP
        sdp: String,
    },
    /// Conveys an answer
    #[serde(rename_all = "camelCase")]
    Answer {
        /// The SDP
        sdp: String,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
/// Contents of the peer message
pub enum PeerMessageInner {
    /// Conveys an ICE candidate
    #[serde(rename_all = "camelCase")]
    Ice {
        /// The candidate string
        candidate: String,
        /// The mline index the candidate applies to
        sdp_m_line_index: u32,
    },
    Sdp(SdpMessage),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Messages directly forwarded from one peer to another
pub struct PeerMessage {
    pub session_id: String,
    #[serde(flatten)]
    pub peer_message: PeerMessageInner,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// End a session
pub struct EndSessionMessage {
    /// The identifier of the session to end
    pub session_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
/// Messages received by the server from peers
pub enum IncomingMessage {
    /// Internal message to let know about new peers
    NewPeer,
    /// Set current peer status
    SetPeerStatus(PeerStatus),
    /// Start a session with another peer
    StartSession(StartSessionMessage),
    /// End an existing session
    EndSession(EndSessionMessage),
    /// Send a message to a peer the sender is currently in session with
    Peer(PeerMessage),
    /// Retrieve the current list of producers
    List,
    /// Retrieve the current list of consumers
    ListConsumers,
}
