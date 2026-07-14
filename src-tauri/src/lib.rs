//! Skill 管理器桌面壳。

use std::path::PathBuf;

use skill_workspace::{SkillWorkspace, WorkspaceSnapshot};
use tauri::Manager;

struct AppState {
    workspace: SkillWorkspace,
}

#[tauri::command]
fn workspace_snapshot(state: tauri::State<'_, AppState>) -> Result<WorkspaceSnapshot, String> {
    state
        .workspace
        .snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn authorize_skill_root(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    state
        .workspace
        .add_root(PathBuf::from(path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rescan_skill_root(
    root_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    state
        .workspace
        .rescan_root(root_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_skill_root(
    root_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    state
        .workspace
        .remove_root(root_id)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let workspace = SkillWorkspace::open(app_data_dir.join("skill-management.sqlite3"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(AppState { workspace });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            workspace_snapshot,
            authorize_skill_root,
            rescan_skill_root,
            remove_skill_root
        ])
        .run(tauri::generate_context!())
        .expect("启动 Skill 管理器失败");
}
