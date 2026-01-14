use crate::models::Account;
use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;
use tracing::{info, warn};

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref();
        info!("Initializing database at: {}", db_path.display());

        let db_url = format!("sqlite:{}", db_path.display());
        let options = SqliteConnectOptions::from_str(&db_url)?.create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("Failed to connect to database")?;

        let db = Self { pool };
        db.run_migrations().await?;

        info!("Database initialized successfully");
        Ok(db)
    }

    async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations");

        let migration_sql = include_str!("../migrations/001_create_accounts_table.sql");
        sqlx::query(migration_sql)
            .execute(&self.pool)
            .await
            .context("Failed to run migrations")?;

        info!("Migrations completed successfully");
        Ok(())
    }

    pub async fn insert_account(&self, account: &Account, batch: Option<&str>) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO accounts (username, password, batch) VALUES (?1, ?2, ?3)
             ON CONFLICT(username) DO UPDATE SET 
             password = excluded.password,
             batch = excluded.batch,
             updated_at = CURRENT_TIMESTAMP",
        )
        .bind(&account.username)
        .bind(&account.password)
        .bind(batch)
        .execute(&self.pool)
        .await
        .context("Failed to insert account")?;

        Ok(result.last_insert_rowid())
    }

    pub async fn insert_accounts(
        &self,
        accounts: &[Account],
        batch: Option<&str>,
    ) -> Result<usize> {
        info!("Inserting {} accounts into database", accounts.len());
        let mut count = 0;

        for account in accounts {
            match self.insert_account(account, batch).await {
                Ok(_) => count += 1,
                Err(e) => {
                    warn!("Failed to insert account {}: {}", account.username, e);
                }
            }
        }

        info!("Successfully inserted {} accounts", count);
        Ok(count)
    }

    pub async fn update_login_result(
        &self,
        username: &str,
        success: bool,
        captcha: Option<&str>,
        two_fa: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE accounts SET 
             success = ?1, 
             captcha = ?2, 
             two_fa = ?3, 
             status = 'completed',
             last_checked_at = CURRENT_TIMESTAMP,
             updated_at = CURRENT_TIMESTAMP
             WHERE username = ?4",
        )
        .bind(success)
        .bind(captcha)
        .bind(two_fa)
        .bind(username)
        .execute(&self.pool)
        .await
        .context("Failed to update login result")?;

        Ok(())
    }

    pub async fn get_account_count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM accounts")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }

    pub async fn get_all_accounts(&self) -> Result<Vec<Account>> {
        let rows = sqlx::query(
            "SELECT username, password, success, captcha, two_fa, batch FROM accounts ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        let accounts = rows
            .into_iter()
            .map(|row| Account {
                username: row.get("username"),
                password: row.get("password"),
                success: row.get("success"),
                captcha: row.get("captcha"),
                two_fa: row.get("two_fa"),
                batch: row.get("batch"),
            })
            .collect();

        Ok(accounts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_database_creation() {
        let temp_db = NamedTempFile::new().unwrap();
        let db = Database::new(temp_db.path()).await.unwrap();

        let count = db.get_account_count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_insert_account() {
        let temp_db = NamedTempFile::new().unwrap();
        let db = Database::new(temp_db.path()).await.unwrap();

        let account = Account::new("test@example.com".to_string(), "password123".to_string());

        let id = db.insert_account(&account, Some("batch1")).await.unwrap();
        assert!(id > 0);

        let count = db.get_account_count().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_insert_duplicate_account_updates() {
        let temp_db = NamedTempFile::new().unwrap();
        let db = Database::new(temp_db.path()).await.unwrap();

        let account1 = Account::new("test@example.com".to_string(), "password123".to_string());

        db.insert_account(&account1, None).await.unwrap();

        let account2 = Account::new("test@example.com".to_string(), "newpassword456".to_string());

        db.insert_account(&account2, None).await.unwrap();

        let count = db.get_account_count().await.unwrap();
        assert_eq!(count, 1);

        let accounts = db.get_all_accounts().await.unwrap();
        assert_eq!(accounts[0].password, "newpassword456");
    }

    #[tokio::test]
    async fn test_insert_multiple_accounts() {
        let temp_db = NamedTempFile::new().unwrap();
        let db = Database::new(temp_db.path()).await.unwrap();

        let accounts = vec![
            Account::new("user1@test.com".to_string(), "pass1".to_string()),
            Account::new("user2@test.com".to_string(), "pass2".to_string()),
            Account::new("user3@test.com".to_string(), "pass3".to_string()),
        ];

        let count = db
            .insert_accounts(&accounts, Some("batch_multiple"))
            .await
            .unwrap();
        assert_eq!(count, 3);

        let total = db.get_account_count().await.unwrap();
        assert_eq!(total, 3);
    }

    #[tokio::test]
    async fn test_get_all_accounts() {
        let temp_db = NamedTempFile::new().unwrap();
        let db = Database::new(temp_db.path()).await.unwrap();

        let accounts = vec![
            Account::new("user1@test.com".to_string(), "pass1".to_string()),
            Account::new("user2@test.com".to_string(), "pass2".to_string()),
        ];

        db.insert_accounts(&accounts, None).await.unwrap();

        let retrieved = db.get_all_accounts().await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].username, "user1@test.com");
        assert_eq!(retrieved[1].username, "user2@test.com");
    }
}
