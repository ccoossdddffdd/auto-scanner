use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio_native_tls::TlsConnector;
use tracing::info;

pub type ImapSession = async_imap::Session<tokio_native_tls::TlsStream<TcpStream>>;

pub struct ImapClient {
    server: String,
    port: u16,
    username: String,
    password: String,
}

impl ImapClient {
    pub fn new(server: String, port: u16, username: String, password: String) -> Self {
        Self {
            server,
            port,
            username,
            password,
        }
    }

    pub async fn connect(&self) -> Result<ImapSession> {
        info!("Connecting to IMAP server...");
        let tcp_stream = TcpStream::connect((self.server.as_str(), self.port))
            .await
            .context("Failed to connect to IMAP server (TCP)")?;

        let native_tls = native_tls::TlsConnector::builder()
            .build()
            .context("Failed to create TLS connector")?;
        let connector = TlsConnector::from(native_tls);

        let tls_stream = connector
            .connect(&self.server, tcp_stream)
            .await
            .context("Failed to establish TLS connection")?;

        let client = async_imap::Client::new(tls_stream);

        let session = client
            .login(&self.username, &self.password)
            .await
            .map_err(|e| e.0)
            .context("IMAP authentication failed")?;

        info!("Successfully connected to IMAP server");
        Ok(session)
    }
}
