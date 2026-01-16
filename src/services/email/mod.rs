pub mod attachment;
pub mod config;
pub mod imap_service;
pub mod monitor;
pub mod notification;
pub mod processor;
pub mod parser;
pub mod sender;
pub mod tracker;

// Re-exports for backward compatibility
pub use attachment::Attachment;
pub use config::EmailConfig;
pub use monitor::EmailMonitor;
