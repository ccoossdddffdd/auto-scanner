use crate::services::email::imap_service::ImapService;
use anyhow::{Context, Result};
use async_imap::types::Mailbox;
use async_trait::async_trait;
use futures::StreamExt;
use tokio::net::TcpStream;
use tokio_native_tls::TlsConnector;
use tracing::info;

pub type ImapSession = async_imap::Session<tokio_native_tls::TlsStream<TcpStream>>;

pub struct ImapClient {
    server: String,
    port: u16,
    username: String,
    password: String,
    session: Option<ImapSession>,
}

impl ImapClient {
    pub fn new(server: String, port: u16, username: String, password: String) -> Self {
        Self {
            server,
            port,
            username,
            password,
            session: None,
        }
    }
}

#[async_trait]
impl ImapService for ImapClient {
    async fn connect(&mut self) -> Result<()> {
        if self.session.is_some() {
            return Ok(());
        }

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
        self.session = Some(session);
        Ok(())
    }

    async fn logout(&mut self) -> Result<()> {
        if let Some(mut session) = self.session.take() {
            session.logout().await.context("Failed to logout")?;
        }
        Ok(())
    }

    async fn select_mailbox(&mut self, mailbox: &str) -> Result<Mailbox> {
        let session = self.session.as_mut().context("IMAP session not connected")?;
        session.select(mailbox).await.context("Failed to select mailbox")
    }

    async fn search_unseen(&mut self) -> Result<Vec<u32>> {
        let session = self.session.as_mut().context("IMAP session not connected")?;
        let result = session.search("UNSEEN").await.context("Failed to search unseen")?;
        Ok(result.into_iter().collect())
    }

    async fn fetch_email(&mut self, uid: u32) -> Result<Option<Vec<u8>>> {
        let session = self.session.as_mut().context("IMAP session not connected")?;
        let mut fetch_stream = session.fetch(uid.to_string(), "RFC822").await?;
        
        if let Some(msg) = fetch_stream.next().await {
            let msg = msg?;
            return Ok(msg.body().map(|b| b.to_vec()));
        }
        Ok(None)
    }

    async fn mark_as_read(&mut self, uid: u32) -> Result<()> {
        let session = self.session.as_mut().context("IMAP session not connected")?;
        let mut stream = session.store(uid.to_string(), "+FLAGS (\\Seen)").await?;
        while let Some(res) = stream.next().await {
            res?;
        }
        Ok(())
    }

    async fn move_email(&mut self, uid: u32, dest: &str) -> Result<()> {
        let session = self.session.as_mut().context("IMAP session not connected")?;
        session.mv(uid.to_string(), dest).await?;
        Ok(())
    }
}
