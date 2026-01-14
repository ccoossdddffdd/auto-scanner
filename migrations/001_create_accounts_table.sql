-- Create accounts table
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    success BOOLEAN,
    captcha TEXT,
    two_fa TEXT,
    batch TEXT,
    last_checked_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create index on username for faster lookups
CREATE INDEX IF NOT EXISTS idx_accounts_username ON accounts(username);

-- Create index on status for filtering
CREATE INDEX IF NOT EXISTS idx_accounts_status ON accounts(status);

-- Create index on batch for filtering
CREATE INDEX IF NOT EXISTS idx_accounts_batch ON accounts(batch);
