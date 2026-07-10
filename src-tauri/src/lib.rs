pub mod commands;
pub mod connectors;
pub mod db;
pub mod error;
pub mod indexer;
pub mod llm;
pub mod models;
pub mod triggers;

use std::path::PathBuf;
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{Emitter, Manager};
use triggers::snapshot::SnapshotStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Every `log::warn!`/`log::info!`/etc. call in this codebase was
        // previously going nowhere — no logger was ever installed, so the
        // `log` crate's default no-op backend silently dropped every
        // record. This both makes those calls actually do something and
        // gives "Copy Diagnostic Report" (see commands::window) a file to
        // read from.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            // Resolve DB path: <app_data>/atelier.db
            let db_path: PathBuf = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir")
                .join("atelier.db");

            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let database = db::open(&db_path).expect("Failed to open database");
            app.manage(database);
            app.manage(std::sync::Arc::new(SnapshotStore::new()));

            // Load permission config from bundled resources
            if let Ok(resource_dir) = app.path().resource_dir() {
                if let Err(e) = llm::permissions::load_config(&resource_dir) {
                    log::warn!("Failed to load permission config: {e}");
                }
            }

            // Tauri's default menu includes a "Preferences…" item on macOS
            // that's inert unless wired up; build an explicit menu instead
            // so it actually opens Atelier's in-app Settings screen
            // (frontend listens for "menu://preferences", see App.tsx).
            let preferences = MenuItemBuilder::with_id("preferences", "Preferences…")
                .accelerator("CmdOrCtrl+,")
                .build(app)?;
            let check_updates =
                MenuItemBuilder::with_id("check_updates", "Check for Updates…").build(app)?;
            let copy_logs =
                MenuItemBuilder::with_id("copy_logs", "Copy Diagnostic Report").build(app)?;
            let app_submenu = SubmenuBuilder::new(app, "Open Atelier")
                .item(&PredefinedMenuItem::about(app, None, None)?)
                .item(&check_updates)
                .separator()
                .item(&preferences)
                .item(&copy_logs)
                .separator()
                .item(&PredefinedMenuItem::quit(app, None)?)
                .build()?;
            let edit_submenu = SubmenuBuilder::new(app, "Edit")
                .item(&PredefinedMenuItem::undo(app, None)?)
                .item(&PredefinedMenuItem::redo(app, None)?)
                .separator()
                .item(&PredefinedMenuItem::cut(app, None)?)
                .item(&PredefinedMenuItem::copy(app, None)?)
                .item(&PredefinedMenuItem::paste(app, None)?)
                .item(&PredefinedMenuItem::select_all(app, None)?)
                .build()?;
            let window_submenu = SubmenuBuilder::new(app, "Window")
                .item(&PredefinedMenuItem::minimize(app, None)?)
                .item(&PredefinedMenuItem::close_window(app, None)?)
                .build()?;
            let menu = MenuBuilder::new(app)
                .item(&app_submenu)
                .item(&edit_submenu)
                .item(&window_submenu)
                .build()?;
            app.set_menu(menu)?;

            app.on_menu_event(|app, event| {
                if event.id() == "preferences" {
                    let _ = app.emit("menu://preferences", ());
                }
                if event.id() == "check_updates" {
                    let _ = app.emit("menu://check_updates", ());
                }
                if event.id() == "copy_logs" {
                    let _ = app.emit("menu://copy_logs", ());
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Profile
            commands::profile::profile_list,
            commands::profile::profile_create,
            commands::profile::profile_update,
            commands::profile::profile_delete,
            commands::profile::profile_switch,
            commands::profile::profile_get_active,
            commands::profile::profile_recreate_dir,
            // Workspace
            commands::workspace::workspace_open,
            commands::workspace::workspace_list,
            commands::workspace::workspace_set_parent,
            commands::workspace::workspace_set_description,
            commands::workspace::workspace_close,
            commands::workspace::workspace_rename,
            commands::workspace::workspace_delete,
            commands::workspace::workspace_relocate,
            commands::workspace::workspace_suggest_name,
            // Files
            commands::files::file_list_tree,
            commands::files::file_create,
            commands::files::image_attachment_save,
            commands::files::file_rename,
            commands::files::file_delete,
            commands::files::file_write,
            commands::files::file_read_raw,
            commands::files::file_read_office_preview,
            commands::files::file_export_pdf,
            commands::files::index_start,
            commands::files::index_cancel,
            commands::files::index_status,
            // Chat
            commands::chat::conversation_list,
            commands::chat::conversation_create,
            commands::chat::conversation_rename,
            commands::chat::conversation_delete,
            commands::chat::conversation_archive,
            commands::chat::conversation_get,
            commands::chat::conversation_compress,
            commands::chat::conversation_fork,
            commands::conversation_groups::conversation_group_list,
            commands::conversation_groups::conversation_group_create,
            commands::conversation_groups::conversation_group_rename,
            commands::conversation_groups::conversation_group_delete,
            commands::conversation_groups::conversation_group_reorder,
            commands::conversation_groups::conversation_set_group,
            commands::chat::ask,
            commands::chat::tool_list,
            commands::chat::tool_approve,
            commands::chat::tool_reject,
            commands::chat::search_hybrid,
            // Settings
            commands::settings::key_save,
            commands::settings::key_delete,
            commands::settings::key_get,
            commands::settings::key_test,
            commands::settings::key_test_with_value,
            commands::settings::key_test_profile,
            commands::settings::key_list_status,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::cred_save,
            commands::settings::cred_delete,
            commands::settings::cred_get,
            commands::settings::cred_get_with_backend,
            commands::settings::cred_get_masked,
            commands::settings::cred_save_profile,
            commands::settings::cred_delete_profile,
            commands::settings::cred_get_profile,
            commands::settings::cred_get_with_backend_profile,
            commands::settings::key_list_status_profile,
            commands::settings::cred_migrate_to_profile,
            commands::settings::factory_reset,
            // Window / platform
            commands::window::platform_info,
            commands::window::open_path,
            commands::window::diagnostic_report,
            llm::skills::skill_list,
            llm::skills::default_skill_list,
            llm::skills::default_skill_install,
            // LLM Functions: permissions
            llm::permissions::get_permission_config,
            llm::permissions::get_permission_level,
            llm::permissions::set_permission_level,
            // LLM Functions: undo
            commands::chat::undo_trigger,
            commands::connectors::connector_github_test,
            commands::connectors::connector_notion_test,
            commands::connectors::connector_slack_test,
            commands::connectors::connector_google_drive_test,
            commands::connectors::connector_github_oauth_start,
            commands::connectors::connector_github_oauth_finish,
            commands::connectors::connector_google_drive_oauth_connect,
            // Plans
            commands::plan::plan_list,
            commands::plan::plan_execute_next,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
