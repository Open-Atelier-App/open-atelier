use tauri::{AppHandle, Emitter, State};
use crate::db::{Db, now_ms};
use crate::error::{AtelierError, Result};
use crate::models::{Plan, PlanTask, PlanWithTasks};

fn row_to_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<Plan> {
    Ok(Plan {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn row_to_plan_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanTask> {
    Ok(PlanTask {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        seq: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        summary: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn fetch_tasks(db: &rusqlite::Connection, plan_id: i64) -> rusqlite::Result<Vec<PlanTask>> {
    let mut stmt = db.prepare(
        "SELECT id, plan_id, seq, description, status, summary, created_at, updated_at
         FROM plan_tasks WHERE plan_id = ?1 ORDER BY seq ASC"
    )?;
    let tasks = stmt.query_map([plan_id], row_to_plan_task)?.collect();
    tasks
}

/// Creates a plan and its tasks from a PLAN trigger (see triggers::parser
/// and resources/skills/llm-functions-v1.md) — called directly from
/// commands::chat's turn-processing loop, which already holds the DB lock
/// context, rather than being its own trigger executed through
/// triggers::executor (that module only knows about the workspace
/// filesystem, not the SQLite conversation state a plan lives in).
pub fn create_plan(db: &Db, conversation_id: i64, title: &str, task_descriptions: &[String]) -> Result<PlanWithTasks> {
    let now = now_ms();
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;

    db.execute(
        "INSERT INTO plans (conversation_id, title, status, created_at) VALUES (?1, ?2, 'pending', ?3)",
        rusqlite::params![conversation_id, title, now],
    )?;
    let plan_id = db.last_insert_rowid();

    for (i, desc) in task_descriptions.iter().enumerate() {
        db.execute(
            "INSERT INTO plan_tasks (plan_id, seq, description, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'pending', ?4, ?4)",
            rusqlite::params![plan_id, i as i64, desc, now],
        )?;
    }

    let plan = db.query_row(
        "SELECT id, conversation_id, title, status, created_at FROM plans WHERE id = ?1",
        [plan_id], row_to_plan,
    )?;
    let tasks = fetch_tasks(&db, plan_id)?;

    Ok(PlanWithTasks { plan, tasks })
}

#[tauri::command]
pub fn plan_list(conversation_id: i64, db: State<Db>) -> Result<Vec<PlanWithTasks>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let mut stmt = db.prepare(
        "SELECT id, conversation_id, title, status, created_at FROM plans WHERE conversation_id = ?1 ORDER BY created_at ASC"
    )?;
    let plans: Vec<Plan> = stmt.query_map([conversation_id], row_to_plan)?.collect::<rusqlite::Result<_>>()?;

    let mut result = Vec::with_capacity(plans.len());
    for plan in plans {
        let tasks = fetch_tasks(&db, plan.id)?;
        result.push(PlanWithTasks { plan, tasks });
    }
    Ok(result)
}

/// Pure DB half of `complete_task` below, split out so it's testable
/// without a live `AppHandle` (which needs a running Tauri app to
/// construct). Returns the updated task and its parent plan so the caller
/// can emit events from them.
fn complete_task_db(conn: &rusqlite::Connection, task_id: i64, success: bool, summary: &str) -> rusqlite::Result<(PlanTask, Plan)> {
    let now = now_ms();

    let status = if success { "done" } else { "failed" };
    let truncated: String = summary.chars().take(280).collect();
    conn.execute(
        "UPDATE plan_tasks SET status = ?1, summary = ?2, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![status, truncated, now, task_id],
    )?;

    let task = conn.query_row(
        "SELECT id, plan_id, seq, description, status, summary, created_at, updated_at FROM plan_tasks WHERE id = ?1",
        [task_id], row_to_plan_task,
    )?;

    let plan_status = if !success {
        "failed"
    } else {
        let remaining: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plan_tasks WHERE plan_id = ?1 AND status != 'done'",
            [task.plan_id], |r| r.get(0),
        )?;
        if remaining == 0 { "done" } else { "running" }
    };
    conn.execute("UPDATE plans SET status = ?1 WHERE id = ?2", rusqlite::params![plan_status, task.plan_id])?;

    let plan = conn.query_row(
        "SELECT id, conversation_id, title, status, created_at FROM plans WHERE id = ?1",
        [task.plan_id], row_to_plan,
    )?;

    Ok((task, plan))
}

/// Marks the given task done/failed with a short summary, and rolls the
/// parent plan's status up from its tasks — called from the end of
/// commands::chat::run_turn once a plan-step turn (including any
/// auto-continuations) has fully settled.
pub fn complete_task(db: &Db, app: &AppHandle, task_id: i64, success: bool, summary: &str) {
    let Ok(conn) = db.lock() else { return };
    let Ok((task, plan)) = complete_task_db(&conn, task_id, success, summary) else { return };
    drop(conn);

    let _ = app.emit("plan://task_updated", &task);
    let _ = app.emit("plan://updated", &plan);
}

/// Kicks off the next pending task in a plan as one conversation turn (see
/// commands::chat::run_turn) — fire-and-forget, like `ask`: this returns as
/// soon as the turn has started streaming, and the frontend learns when the
/// step actually finishes via the `plan://task_updated`/`plan://updated`
/// events emitted by `complete_task` above. Callers decide whether to chain
/// straight into the next step (auto-run) or wait for the user.
#[tauri::command]
pub async fn plan_execute_next(plan_id: i64, db: State<'_, Db>, app: AppHandle) -> Result<Option<PlanTask>> {
    let db_owned = db.inner().clone();

    let (conversation_id, provider, model, next_task, plan_title) = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;

        let (conversation_id, title): (i64, String) = db.query_row(
            "SELECT conversation_id, title FROM plans WHERE id = ?1",
            [plan_id], |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|_| AtelierError::not_found("Plan not found"))?;

        let (provider, model): (Option<String>, Option<String>) = db.query_row(
            "SELECT provider, model FROM conversations WHERE id = ?1",
            [conversation_id], |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|_| AtelierError::not_found("Conversation not found"))?;

        let next_task = db.query_row(
            "SELECT id, plan_id, seq, description, status, summary, created_at, updated_at
             FROM plan_tasks WHERE plan_id = ?1 AND status = 'pending' ORDER BY seq ASC LIMIT 1",
            [plan_id], row_to_plan_task,
        ).ok();

        (conversation_id, provider, model, next_task, title)
    };

    let Some(task) = next_task else {
        // No pending tasks left; nothing to run. Frontend can inspect the
        // plan's rolled-up status (already correct from the last completed
        // task) to know whether it finished cleanly or stopped on a failure.
        return Ok(None);
    };

    let (Some(provider), Some(model)) = (provider, model) else {
        return Err(AtelierError::new(
            crate::error::ErrorCode::Unsupported,
            "This conversation has no provider/model set yet — send a regular message first",
        ));
    };

    let now = now_ms();
    {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        db.execute("UPDATE plan_tasks SET status = 'running', updated_at = ?1 WHERE id = ?2", rusqlite::params![now, task.id])?;
        db.execute("UPDATE plans SET status = 'running' WHERE id = ?1", rusqlite::params![plan_id])?;
    }
    let running_task = PlanTask { status: "running".to_string(), updated_at: now, ..task.clone() };
    let _ = app.emit("plan://task_updated", &running_task);

    let step_instruction = format!(
        "Execute step {} of the plan \"{plan_title}\": {}\n\nWhen finished, send a short MESSAGE summarizing what you did.",
        task.seq + 1, task.description,
    );

    super::chat::run_turn(conversation_id, step_instruction, provider, model, db_owned, app, Some(task.id)).await?;

    Ok(Some(task))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_conversation() -> (Db, i64) {
        let db = crate::db::open_in_memory().unwrap();
        let now = now_ms();
        let conv_id = {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', ?1, ?1)",
                rusqlite::params![now],
            ).unwrap();
            let profile_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO workspaces (profile_id, path, name, created_at, last_opened_at) VALUES (?1, '/tmp/ws', 'ws', ?2, ?2)",
                rusqlite::params![profile_id, now],
            ).unwrap();
            let workspace_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO conversations (workspace_id, title, created_at, updated_at, provider, model) VALUES (?1, 'conv', ?2, ?2, 'openai', 'gpt')",
                rusqlite::params![workspace_id, now],
            ).unwrap();
            conn.last_insert_rowid()
        };
        (db, conv_id)
    }

    #[test]
    fn create_plan_inserts_plan_and_ordered_tasks() {
        let (db, conv_id) = setup_conversation();
        let tasks = vec!["Step A".to_string(), "Step B".to_string(), "Step C".to_string()];
        let created = create_plan(&db, conv_id, "My Plan", &tasks).unwrap();

        assert_eq!(created.plan.title, "My Plan");
        assert_eq!(created.plan.status, "pending");
        assert_eq!(created.tasks.len(), 3);
        assert_eq!(created.tasks[0].seq, 0);
        assert_eq!(created.tasks[0].description, "Step A");
        assert_eq!(created.tasks[2].description, "Step C");
        assert!(created.tasks.iter().all(|t| t.status == "pending"));
    }

    #[test]
    fn complete_task_db_marks_task_done_and_rolls_plan_status() {
        let (db, conv_id) = setup_conversation();
        let created = create_plan(&db, conv_id, "Plan", &["Only step".to_string()]).unwrap();
        let task_id = created.tasks[0].id;

        let conn = db.lock().unwrap();
        let (task, plan) = complete_task_db(&conn, task_id, true, "Did the thing").unwrap();
        assert_eq!(task.status, "done");
        assert_eq!(task.summary.as_deref(), Some("Did the thing"));
        // The only task just finished, so the plan as a whole is done too.
        assert_eq!(plan.status, "done");
    }

    #[test]
    fn complete_task_db_keeps_plan_running_with_remaining_tasks() {
        let (db, conv_id) = setup_conversation();
        let created = create_plan(&db, conv_id, "Plan", &["First".to_string(), "Second".to_string()]).unwrap();
        let first_id = created.tasks[0].id;

        let conn = db.lock().unwrap();
        let (_task, plan) = complete_task_db(&conn, first_id, true, "done first").unwrap();
        // One task (of two) is done — the plan isn't finished yet.
        assert_eq!(plan.status, "running");
    }

    #[test]
    fn complete_task_db_marks_plan_failed_on_failure() {
        let (db, conv_id) = setup_conversation();
        let created = create_plan(&db, conv_id, "Plan", &["First".to_string(), "Second".to_string()]).unwrap();
        let first_id = created.tasks[0].id;

        let conn = db.lock().unwrap();
        let (task, plan) = complete_task_db(&conn, first_id, false, "it broke").unwrap();
        assert_eq!(task.status, "failed");
        assert_eq!(plan.status, "failed");
    }

    #[test]
    fn plan_list_returns_plans_with_their_tasks() {
        let (db, conv_id) = setup_conversation();
        create_plan(&db, conv_id, "Plan A", &["a1".to_string()]).unwrap();
        create_plan(&db, conv_id, "Plan B", &["b1".to_string(), "b2".to_string()]).unwrap();

        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, conversation_id, title, status, created_at FROM plans WHERE conversation_id = ?1 ORDER BY created_at ASC").unwrap();
        let plans: Vec<Plan> = stmt.query_map([conv_id], row_to_plan).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].title, "Plan A");
        assert_eq!(plans[1].title, "Plan B");

        let tasks_b = fetch_tasks(&conn, plans[1].id).unwrap();
        assert_eq!(tasks_b.len(), 2);
    }
}
