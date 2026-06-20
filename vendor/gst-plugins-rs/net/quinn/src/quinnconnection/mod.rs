use crate::common::*;
use crate::utils::*;
use gst::glib;
use quinn::Connection;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, LazyLock, Mutex, OnceLock},
};
use web_transport_quinn::Session;

pub const QUINN_CONNECTION_CONTEXT: &str = "gst.quinn.connection";

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "GstQuinnConnectionContext")]
pub struct QuinnConnectionContext(pub Arc<QuinnConnectionContextInner>);

#[derive(Clone, Debug)]
pub enum QuinnConnection {
    WebTransport(Session),
    Quic(Connection),
}

#[derive(Debug)]
pub struct QuinnConnectionContextInner {
    pub connection: QuinnConnection,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnconnection",
        gst::DebugColorFlags::empty(),
        Some("Quinn Connection (shared)"),
    )
});

static SHARED_CONNECTION_STATE: OnceLock<Mutex<HashMap<SocketAddr, SharedConnection>>> =
    OnceLock::new();

#[derive(Debug, Clone)]
pub struct SharedConnection {
    endpoint_config: Option<QuinnQuicEndpointConfig>,
    inner: Arc<Mutex<Option<QuinnConnection>>>,
}

impl SharedConnection {
    pub fn get_or_init(addr: SocketAddr) -> Self {
        SHARED_CONNECTION_STATE
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .entry(addr)
            .or_insert_with_key(|_| SharedConnection {
                endpoint_config: None,
                inner: Arc::new(Mutex::new(None)),
            })
            .clone()
    }

    pub fn set_endpoint_config(&mut self, endpoint_config: QuinnQuicEndpointConfig) {
        if self.endpoint_config.is_some() {
            return;
        }

        self.endpoint_config = Some(endpoint_config);
    }

    pub async fn connect(
        &mut self,
        role: QuinnQuicRole,
        url: Option<url::Url>,
    ) -> Result<(), WaitError> {
        if self.endpoint_config.is_none() {
            return Err(WaitError::FutureError(gst::error_msg!(
                gst::ResourceError::Failed,
                ["QuinnQuicEndpointConfig not set"]
            )));
        }

        let endpoint_config = self.endpoint_config.as_ref().unwrap();

        let endpoint = match role {
            QuinnQuicRole::Server => server_endpoint(endpoint_config).map_err(|err| {
                WaitError::FutureError(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Failed to configure endpoint: {}", err]
                ))
            })?,
            QuinnQuicRole::Client => client_endpoint(endpoint_config).map_err(|err| {
                WaitError::FutureError(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Failed to configure endpoint: {}", err]
                ))
            })?,
        };

        let connection = if endpoint_config.webtransport {
            let session = match role {
                QuinnQuicRole::Server => {
                    let mut incoming_conn = web_transport_quinn::Server::new(endpoint);

                    let request = incoming_conn.accept().await.ok_or_else(|| {
                        WaitError::FutureError(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Connection error"]
                        ))
                    })?;

                    // FIXME: We now accept all request without verifying the URL
                    request.ok().await.unwrap()
                }
                QuinnQuicRole::Client => {
                    let url = url.expect("Url should be valid");

                    let client = client(endpoint_config).map_err(|err| {
                        WaitError::FutureError(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Failed to configure endpoint: {}", err]
                        ))
                    })?;

                    client.connect(url).await.map_err(|err| {
                        WaitError::FutureError(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Failed to connect to server: {}", err]
                        ))
                    })?
                }
            };

            gst::info!(
                CAT,
                "Remote session established: {}",
                session.remote_address(),
            );

            QuinnConnection::WebTransport(session)
        } else {
            let connection = match role {
                QuinnQuicRole::Server => {
                    let incoming_conn = endpoint.accept().await.unwrap();

                    incoming_conn.await.map_err(|err| {
                        WaitError::FutureError(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Connection error: {}", err]
                        ))
                    })?
                }
                QuinnQuicRole::Client => endpoint
                    .connect(endpoint_config.server_addr, &endpoint_config.server_name)
                    .unwrap()
                    .await
                    .map_err(|err| {
                        WaitError::FutureError(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Connection error: {}", err]
                        ))
                    })?,
            };

            gst::info!(
                CAT,
                "Remote connection established: {}",
                connection.remote_address(),
            );

            QuinnConnection::Quic(connection)
        };

        let mut inner = self.inner.lock().unwrap();
        *inner = Some(connection);

        Ok(())
    }

    pub fn connection(&self) -> Option<QuinnConnection> {
        self.inner.lock().unwrap().clone()
    }
}
