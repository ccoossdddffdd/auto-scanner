mod cli;

use anyhow::Result;
use auto_scanner::csv_reader::read_accounts_from_csv;
use auto_scanner::database::Database;
use clap::Parser;
use cli::Cli;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .init();

    let cli = Cli::parse();
    
    info!("Starting auto-scanner");
    info!("Input file: {}", cli.input);

    // Initialize database
    let db = Database::new("auto-scanner.db").await?;

    // Read accounts from CSV
    let accounts = read_accounts_from_csv(&cli.input).await?;
    info!("Read {} accounts from CSV file", accounts.len());

    // Insert accounts into database
    let inserted = db.insert_accounts(&accounts).await?;
    info!("Inserted {} accounts into database", inserted);

    // Verify by counting total accounts
    let total = db.get_account_count().await?;
    info!("Total accounts in database: {}", total);

    info!("Auto-scanner completed successfully");
    Ok(())
}
