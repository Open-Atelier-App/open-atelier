//! User-defined folders for organizing a workspace's conversations by
//! subject — see ConversationGroup in models::mod for the shape and
//! ConversationList.tsx for the drag-and-drop UI that drives these.

use crate::db::{now_ms, Db};
use crate::error::{AtelierError, Result};
use crate::models::{Conversation, ConversationGroup};
use tauri::State;

const GROUP_COLUMNS: &str = "id, workspace_id, name, position, created_at";

fn row_to_group(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationGroup> {
    Ok(ConversationGroup {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        name: row.get(2)?,
        position: row.get(3)?,
        created_at: row.get(4)?,
    })
}

#[tauri::command]
pub fn conversation_group_list(workspace_id: i64, db: State<Db>) -> Result<Vec<ConversationGroup>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let mut stmt = db.prepare(
        &format!("SELECT {GROUP_COLUMNS} FROM conversation_groups WHERE workspace_id = ?1 ORDER BY position ASC")
    )?;
    let rows = stmt.query_map([workspace_id], row_to_group)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[tauri::command]
pub fn conversation_group_create(
    workspace_id: i64,
    name: String,
    db: State<Db>,
) -> Result<ConversationGroup> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AtelierError::internal("Group name cannot be empty"));
    }
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let next_position: i64 = db.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM conversation_groups WHERE workspace_id = ?1",
        [workspace_id],
        |r| r.get(0),
    )?;
    let now = now_ms();
    db.execute(
        "INSERT INTO conversation_groups (workspace_id, name, position, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![workspace_id, name, next_position, now],
    )?;
    let id = db.last_insert_rowid();
    let group = db.query_row(
        &format!("SELECT {GROUP_COLUMNS} FROM conversation_groups WHERE id = ?1"),
        [id],
        row_to_group,
    )?;
    Ok(group)
}

#[tauri::command]
pub fn conversation_group_rename(
    id: i64,
    name: String,
    db: State<Db>,
) -> Result<ConversationGroup> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AtelierError::internal("Group name cannot be empty"));
    }
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "UPDATE conversation_groups SET name = ?1 WHERE id = ?2",
        rusqlite::params![name, id],
    )?;
    let group = db.query_row(
        &format!("SELECT {GROUP_COLUMNS} FROM conversation_groups WHERE id = ?1"),
        [id],
        row_to_group,
    )?;
    Ok(group)
}

/// Deletes the group itself, not its conversations — they fall back to
/// ungrouped via the `group_id` column's `ON DELETE SET NULL`.
#[tauri::command]
pub fn conversation_group_delete(id: i64, db: State<Db>) -> Result<()> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute("DELETE FROM conversation_groups WHERE id = ?1", [id])?;
    Ok(())
}

/// Rewrites the position of every group in `ordered_ids` to match its
/// index in that list — the whole reordered sequence for a workspace, sent
/// in one call after a drag-to-reorder, rather than one relative move at a
/// time. Only touches groups that actually belong to `workspace_id`, so a
/// stale/tampered id list can't move another workspace's groups.
#[tauri::command]
pub fn conversation_group_reorder(
    workspace_id: i64,
    ordered_ids: Vec<i64>,
    db: State<Db>,
) -> Result<Vec<ConversationGroup>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    for (position, id) in ordered_ids.iter().enumerate() {
        db.execute(
            "UPDATE conversation_groups SET position = ?1 WHERE id = ?2 AND workspace_id = ?3",
            rusqlite::params![position as i64, id, workspace_id],
        )?;
    }
    let mut stmt = db.prepare(
        &format!("SELECT {GROUP_COLUMNS} FROM conversation_groups WHERE workspace_id = ?1 ORDER BY position ASC")
    )?;
    let rows = stmt.query_map([workspace_id], row_to_group)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Assigns (or, with `group_id: None`, clears) which folder a conversation
/// is filed under — the backend half of drag-and-drop in ConversationList.
#[tauri::command]
pub fn conversation_set_group(
    conversation_id: i64,
    group_id: Option<i64>,
    db: State<Db>,
) -> Result<Conversation> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "UPDATE conversations SET group_id = ?1 WHERE id = ?2",
        rusqlite::params![group_id, conversation_id],
    )?;
    let conv = db.query_row(
        "SELECT id, workspace_id, title, created_at, updated_at, provider, model, summary, compressed_memory, compressed_at, group_id \
         FROM conversations WHERE id = ?1",
        [conversation_id],
        |r| Ok(Conversation {
            id: r.get(0)?,
            workspace_id: r.get(1)?,
            title: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
            provider: r.get(5)?,
            model: r.get(6)?,
            summary: r.get(7)?,
            compressed_memory: r.get(8)?,
            compressed_at: r.get(9)?,
            group_id: r.get(10)?,
        }),
    )?;
    Ok(conv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_workspace(conn: &rusqlite::Connection) -> i64 {
        conn.execute(
            "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'w', '/tmp/w', 0, 0)",
            [],
        ).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn create_assigns_increasing_positions() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        let ws_id = setup_workspace(&conn);
        drop(conn);

        let g1 = conversation_group_create_impl(&db, ws_id, "Research").unwrap();
        let g2 = conversation_group_create_impl(&db, ws_id, "Drafts").unwrap();
        assert_eq!(g1.position, 0);
        assert_eq!(g2.position, 1);
    }

    #[test]
    fn delete_ungroups_its_conversations_instead_of_orphaning_them() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        let ws_id = setup_workspace(&conn);
        conn.execute(
            "INSERT INTO conversations (workspace_id, title, created_at, updated_at) VALUES (?1, 't', 0, 0)",
            [ws_id],
        ).unwrap();
        let conv_id = conn.last_insert_rowid();
        drop(conn);

        let group = conversation_group_create_impl(&db, ws_id, "Research").unwrap();
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "UPDATE conversations SET group_id = ?1 WHERE id = ?2",
                rusqlite::params![group.id, conv_id],
            )
            .unwrap();
        }

        {
            let conn = db.lock().unwrap();
            conn.execute("DELETE FROM conversation_groups WHERE id = ?1", [group.id])
                .unwrap();
        }

        let conn = db.lock().unwrap();
        let group_id: Option<i64> = conn
            .query_row(
                "SELECT group_id FROM conversations WHERE id = ?1",
                [conv_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(group_id, None);
    }

    #[test]
    fn reorder_only_touches_groups_in_the_given_workspace() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        let ws1 = setup_workspace(&conn);
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'w2', '/tmp/w2', 0, 0)",
            [],
        ).unwrap();
        let ws2 = conn.last_insert_rowid();
        drop(conn);

        let a = conversation_group_create_impl(&db, ws1, "A").unwrap();
        let b = conversation_group_create_impl(&db, ws1, "B").unwrap();
        let other = conversation_group_create_impl(&db, ws2, "Other").unwrap();

        // Reverse a/b within ws1; try (and fail, silently) to also move
        // `other` by including its id under the wrong workspace_id.
        let conn = db.lock().unwrap();
        for (position, id) in [b.id, a.id, other.id].iter().enumerate() {
            conn.execute(
                "UPDATE conversation_groups SET position = ?1 WHERE id = ?2 AND workspace_id = ?3",
                rusqlite::params![position as i64, id, ws1],
            )
            .unwrap();
        }
        let other_position: i64 = conn
            .query_row(
                "SELECT position FROM conversation_groups WHERE id = ?1",
                [other.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            other_position, 0,
            "other workspace's group position must be untouched"
        );
    }

    // Test-only helper mirroring conversation_group_create's logic without
    // needing a Tauri State<Db> wrapper.
    fn conversation_group_create_impl(
        db: &Db,
        workspace_id: i64,
        name: &str,
    ) -> Result<ConversationGroup> {
        let conn = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        let next_position: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM conversation_groups WHERE workspace_id = ?1",
            [workspace_id],
            |r| r.get(0),
        )?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO conversation_groups (workspace_id, name, position, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![workspace_id, name, next_position, now],
        )?;
        let id = conn.last_insert_rowid();
        let group = conn.query_row(
            &format!("SELECT {GROUP_COLUMNS} FROM conversation_groups WHERE id = ?1"),
            [id],
            row_to_group,
        )?;
        Ok(group)
    }
}
