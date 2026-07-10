pub mod schema;

use crate::error::{AtelierError, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub type Db = Arc<Mutex<Connection>>;

pub fn open(path: &Path) -> Result<Db> {
    let conn = Connection::open(path).map_err(|e| AtelierError::db(e.to_string()))?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| AtelierError::db(e.to_string()))?;
    run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

pub fn open_in_memory() -> Result<Db> {
    let conn = Connection::open_in_memory().map_err(|e| AtelierError::db(e.to_string()))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| AtelierError::db(e.to_string()))?;
    run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(schema::SCHEMA)
        .map_err(|e| AtelierError::db(format!("Migration failed: {e}")))?;

    // Defensive ALTER TABLEs for upgrading existing DBs created before these
    // columns existed in schema.rs's CREATE TABLE IF NOT EXISTS. New DBs already
    // have these columns from SCHEMA above, so "duplicate column name" errors
    // here are expected and safely ignored.
    add_column_if_missing(conn, "messages", "input_tokens", "INTEGER");
    add_column_if_missing(conn, "messages", "output_tokens", "INTEGER");
    add_column_if_missing(conn, "messages", "display_override", "TEXT");
    add_column_if_missing(
        conn,
        "workspaces",
        "parent_workspace_id",
        "INTEGER REFERENCES workspaces(id) ON DELETE SET NULL",
    );
    add_column_if_missing(conn, "conversations", "summary", "TEXT");
    add_column_if_missing(conn, "workspaces", "description", "TEXT");
    add_column_if_missing(conn, "conversations", "compressed_memory", "TEXT");
    add_column_if_missing(conn, "conversations", "compressed_at", "INTEGER");
    add_column_if_missing(
        conn,
        "conversations",
        "group_id",
        "INTEGER REFERENCES conversation_groups(id) ON DELETE SET NULL",
    );

    Ok(())
}

fn add_column_if_missing(conn: &Connection, table: &str, column: &str, ty: &str) {
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {ty}");
    match conn.execute(&sql, params![]) {
        Ok(_) => {}
        Err(e) => {
            // rusqlite surfaces SQLite's "duplicate column name" as a generic
            // SqliteFailure; matching on the message is the only reliable way
            // to distinguish it from a real failure without a custom error code.
            let msg = e.to_string();
            if !msg.contains("duplicate column name") {
                // Not the expected benign case; log but don't panic — schema
                // mismatches shouldn't crash the whole app at startup.
                eprintln!("warning: failed to add column {table}.{column}: {msg}");
            }
        }
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
