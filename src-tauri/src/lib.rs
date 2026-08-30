mod application;
mod commands;
mod ipc;
mod persistence;
mod state;

use std::fs;

use skill_registry::SkillsShClient;
use state::{AppState, ApplicationHandle};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data)?;
            let application = ApplicationHandle::start(
                app_data.join("yssskills.sqlite3"),
                app_data.join("catalog"),
            )?;
            let registry = SkillsShClient::new()?;
            app.manage(AppState {
                application,
                registry,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::dashboard::get_dashboard_overview,
            commands::skills::list_catalog_skills,
            commands::skills::get_catalog_skill,
            commands::workspaces::get_workspaces_overview,
            commands::workspaces::create_workspace,
            commands::workspaces::observe_workspace,
            commands::workspaces::reconcile_workspace,
            commands::registry::search_registry,
            commands::registry::get_registry_leaderboard,
            commands::settings::get_app_settings,
            commands::settings::update_catalog_root,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("failed to run YssSkills: {error}");
    }
}
