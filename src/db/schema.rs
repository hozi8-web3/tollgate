use anyhow::Result;
use rusqlite::Connection;

/// Run all database migrations.
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS requests (
            id                   TEXT PRIMARY KEY,
            timestamp            TEXT NOT NULL,
            provider             TEXT NOT NULL,
            model                TEXT NOT NULL,
            original_model       TEXT NOT NULL,
            was_substituted      INTEGER NOT NULL DEFAULT 0,
            input_tokens         INTEGER NOT NULL DEFAULT 0,
            output_tokens        INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens    INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens   INTEGER NOT NULL DEFAULT 0,
            input_cost_usd       REAL NOT NULL DEFAULT 0,
            output_cost_usd      REAL NOT NULL DEFAULT 0,
            cache_read_cost_usd  REAL NOT NULL DEFAULT 0,
            cache_write_cost_usd REAL NOT NULL DEFAULT 0,
            total_cost_usd       REAL NOT NULL DEFAULT 0,
            latency_ms           INTEGER NOT NULL DEFAULT 0,
            stop_reason          TEXT,
            task_type            TEXT,
            tags                 TEXT,
            anomaly              INTEGER NOT NULL DEFAULT 0,
            anomaly_reason       TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_ts    ON requests(timestamp);
        CREATE INDEX IF NOT EXISTS idx_model ON requests(model);
        CREATE INDEX IF NOT EXISTS idx_task  ON requests(task_type);
        ",
    )?;

    tracing::debug!("Database migrations complete");
    Ok(())
}
