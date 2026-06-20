// SPDX-License-Identifier: MPL-2.0

use anyhow::Error;
use clap::Parser;
use gst_plugin_webrtc_signalling::handlers::Handler;
use gst_plugin_webrtc_signalling::server::{Server, ServerError};
use std::time::Duration;
use tokio::{net::TcpListener, task};
use tracing::{info, warn};
use tracing_subscriber::prelude::*;

use std::{fs::File, io::BufReader, path::PathBuf, sync::Arc};
use tokio_rustls::{rustls, TlsAcceptor};

const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Parser, Debug)]
#[clap(about, version, author)]
/// Program arguments
struct Args {
    /// Address to listen on
    #[clap(long, default_value = "0.0.0.0")]
    host: String,
    /// Port to listen on
    #[clap(short, long, default_value_t = 8443)]
    port: u16,
    /// TLS certificate to use
    #[clap(short, long)]
    cert: Option<String>,
    /// Private key to use
    #[clap(short, long)]
    key: Option<String>,
}

fn initialize_logging(envvar_name: &str) -> Result<(), Error> {
    tracing_log::LogTracer::init()?;
    let env_filter = tracing_subscriber::EnvFilter::try_from_env(envvar_name)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_ids(true)
        .with_target(true)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        );
    let subscriber = tracing_subscriber::Registry::default()
        .with(env_filter)
        .with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let server = Server::spawn(Handler::new);

    initialize_logging("WEBRTCSINK_SIGNALLING_SERVER_LOG")?;

    let addr = format!("{}:{}", args.host, args.port);

    // Create the event loop and TCP listener we'll accept connections on.
    let listener = TcpListener::bind(&addr).await?;

    let acceptor = if let (Some(cert), Some(key)) = (&args.cert, &args.key) {
        create_tls_acceptor(cert, key).await.ok()
    } else {
        None
    };

    info!("Listening on: {}", addr);

    while let Ok((stream, address)) = listener.accept().await {
        let mut server_clone = server.clone();
        info!("Accepting connection from {}", address);

        if let Some(acceptor) = acceptor.clone() {
            tokio::spawn(async move {
                match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await {
                    Ok(Ok(stream)) => server_clone.accept_async(stream).await,
                    Ok(Err(err)) => {
                        warn!("Failed to accept TLS connection from {}: {}", address, err);
                        Err(ServerError::TLSHandshake(err))
                    }
                    Err(elapsed) => {
                        warn!("TLS connection timed out {} after {}", address, elapsed);
                        Err(ServerError::TLSHandshakeTimeout(elapsed))
                    }
                }
            });
        } else {
            task::spawn(async move { server_clone.accept_async(stream).await });
        }
    }

    Ok(())
}

fn read_certs_from_file(
    certificate_file: PathBuf,
) -> Result<Vec<rustls_pki_types::CertificateDer<'static>>, Box<dyn std::error::Error>> {
    let cert_file = File::open(&certificate_file)?;
    let mut cert_file_rdr = BufReader::new(cert_file);

    let certs_result = rustls_pemfile::certs(&mut cert_file_rdr);
    let mut certs = Vec::new();

    for cert_result in certs_result {
        match cert_result {
            Ok(cert) => certs.push(cert),
            Err(e) => {
                return Err(format!("Failed to parse certificate: {e}").into());
            }
        }
    }

    if certs.is_empty() {
        return Err(format!(
            "No valid certificates found in {}",
            certificate_file.display()
        )
        .into());
    }

    Ok(certs)
}

fn read_private_key_from_file(
    private_key_file: PathBuf,
) -> Result<rustls_pki_types::PrivateKeyDer<'static>, Box<dyn std::error::Error>> {
    let key_file = File::open(&private_key_file)?;
    let mut key_file_rdr = BufReader::new(key_file);
    let items_result = rustls_pemfile::read_all(&mut key_file_rdr);

    for item_result in items_result {
        let item = item_result.map_err(|e| {
            format!(
                "Failed to parse PEM item in {}: {e}",
                private_key_file.display(),
            )
        })?;

        match item {
            rustls_pemfile::Item::Pkcs1Key(key) => {
                return Ok(rustls_pki_types::PrivateKeyDer::from(key));
            }
            rustls_pemfile::Item::Pkcs8Key(key) => {
                return Ok(rustls_pki_types::PrivateKeyDer::from(key));
            }
            rustls_pemfile::Item::Sec1Key(key) => {
                return Ok(rustls_pki_types::PrivateKeyDer::from(key));
            }
            _ => continue,
        }
    }

    Err(format!(
        "No valid private key found in {}",
        private_key_file.display()
    )
    .into())
}

pub async fn create_tls_acceptor(
    certificate_file: &str,
    private_key_file: &str,
) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let ring_provider = rustls::crypto::ring::default_provider();
    let certs = read_certs_from_file(certificate_file.into())?;
    let key = read_private_key_from_file(private_key_file.into())?;

    let config = rustls::ServerConfig::builder_with_provider(ring_provider.into())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}
