pub mod schema;
pub mod write;
pub mod read;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Thread-safe database handle.
pub type DbPool = Arc<Mutex<Connection>>;

/// Initialize the database connection and run migrations.
pub fn init(db_path: &Path) -> Result<DbPool> {
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

    // Run schema migrations
    schema::run_migrations(&conn)?;

    tracing::info!("Database initialized at {}", db_path.display());
    Ok(Arc::new(Mutex::new(conn)))
}
