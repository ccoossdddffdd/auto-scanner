mod cli;

use anyhow::{Context, Result};
use auto_scanner::browser::{playwright_adapter::PlaywrightAdapter, BrowserAdapter};
use auto_scanner::csv_reader::read_accounts_from_csv;
use auto_scanner::database::Database;

use clap::Parser;
use cli::Cli;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let cli = Cli::parse();

    info!("Starting auto-scanner");
    info!("Input file: {}", cli.input);
    info!("Browser backend: {}", cli.backend);
    info!("Remote URL: {}", cli.remote_url);

    // Initialize database
    let db = Database::new("auto-scanner.db").await?;

    // Read accounts from CSV
    let accounts = read_accounts_from_csv(&cli.input).await?;
    info!("Read {} accounts from CSV file", accounts.len());

    // Insert accounts into database
    let inserted = db.insert_accounts(&accounts).await?;
    info!("Inserted {} accounts into database", inserted);

    // Initialize browser adapter
    let adapter: Box<dyn BrowserAdapter> = match cli.backend.as_str() {
        "playwright" | "cdp" => {
            info!("Connecting to CDP at {}", cli.remote_url);
            Box::new(
                PlaywrightAdapter::new(&cli.remote_url)
                    .await
                    .context("Failed to initialize Playwright adapter")?,
            )
        }
        _ => anyhow::bail!("Unsupported backend: {}", cli.backend),
    };

    // Try to login with accounts
    // For now, we just process the first one to verify the flow
    if let Some(account) = accounts.first() {
        info!("Verifying login flow for account: {}", account.username);

        if let Err(e) = perform_login(adapter.as_ref(), account).await {
            error!("Login failed: {}", e);
        } else {
            info!("Login flow executed successfully");
        }
    }

    info!("Auto-scanner completed successfully");
    Ok(())
}

async fn perform_login(
    adapter: &dyn BrowserAdapter,
    account: &auto_scanner::models::Account,
) -> Result<()> {
    info!("Navigating to Facebook...");
    adapter.navigate("https://www.facebook.com").await?;

    info!("Waiting for email input...");
    // Facebook selectors might change, these are common ones
    // Try to find email input
    adapter.wait_for_element("input[name='email']").await?;

    info!("Typing credentials...");
    adapter
        .type_text("input[name='email']", &account.username)
        .await?;
    adapter
        .type_text("input[name='pass']", &account.password)
        .await?;

    info!("Clicking login button...");
    adapter.click("button[name='login']").await?;
    
    // Wait a bit for navigation/error
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    info!("Taking screenshot...");
    adapter.take_screenshot("login_attempt.png").await?;

    Ok(())
}
