mod agent_config;
mod application;
mod commands;
mod ipc;
mod persistence;
mod state;

use std::{
    fs,
    path::{Path, PathBuf},
};

use skill_registry::SkillsShClient;
use state::{AppState, ApplicationHandle};
use tauri::Manager;

fn default_catalog_root(home_dir: &Path) -> PathBuf {
    home_dir.join(".yss-skills")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let catalog_root = default_catalog_root(&app.path().home_dir()?);
            fs::create_dir_all(&app_data)?;
            let application =
                ApplicationHandle::start(app_data.join("yssskills.sqlite3"), catalog_root)?;
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
            commands::skills::scan_import_folder,
            commands::skills::import_local_skills,
            commands::skills::export_catalog_skills,
            commands::skills::delete_catalog_skills,
            commands::skills::get_catalog_skill,
            commands::workspaces::get_workspaces_overview,
            commands::workspaces::detect_agents,
            commands::workspaces::add_detected_agents,
            commands::workspaces::delete_agents,
            commands::workspaces::copy_project_agent_skills,
            commands::workspaces::delete_project_agents,
            commands::workspaces::create_workspace,
            commands::workspaces::save_agent,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_root_is_yss_skills_under_home_directory() {
        let home_dir = Path::new("C:/Users/example");

        assert_eq!(default_catalog_root(home_dir), home_dir.join(".yss-skills"));
    }
}
