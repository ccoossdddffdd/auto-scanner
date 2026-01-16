use anyhow::Result;
use async_trait::async_trait;
use async_imap::types::Mailbox;

#[async_trait]
pub trait ImapService: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn logout(&mut self) -> Result<()>;
    async fn select_mailbox(&mut self, mailbox: &str) -> Result<Mailbox>;
    async fn search_unseen(&mut self) -> Result<Vec<u32>>;
    async fn fetch_email(&mut self, uid: u32) -> Result<Option<Vec<u8>>>;
    async fn mark_as_read(&mut self, uid: u32) -> Result<()>;
    async fn move_email(&mut self, uid: u32, dest: &str) -> Result<()>;
}
