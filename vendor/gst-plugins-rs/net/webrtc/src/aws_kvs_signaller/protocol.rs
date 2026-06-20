// SPDX-License-Identifier: MPL-2.0

/// The default protocol used by the signalling server
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SdpOffer {
    #[serde(rename = "type")]
    pub type_: String,
    pub sdp: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: String,
    pub sdp_m_line_index: u32,
    pub username_fragment: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IncomingMessage {
    pub message_type: String,
    pub message_payload: String,
    pub sender_client_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SdpAnswer {
    #[serde(rename = "type")]
    pub type_: String,
    pub sdp: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingIceCandidate {
    pub candidate: String,
    pub sdp_mid: String,
    pub sdp_m_line_index: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingMessage {
    pub action: String,
    pub message_payload: String,
    pub recipient_client_id: String,
}
