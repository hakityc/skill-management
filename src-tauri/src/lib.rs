//! Skill 管理器桌面壳。

use std::path::PathBuf;

use skill_workspace::{
    DuplicateDecisionKind, DuplicateDecisionRecord, DuplicateReview, SkillChangeOutcome,
    SkillChangePlan, SkillChangeRecord, SkillDetail, SkillDraft, SkillDraftValidation,
    SkillFilePreview, SkillOrganizationChange, SkillOrganizationSnapshot, SkillQuery,
    SkillSearchResult, SkillWorkspace, SkillWorkspaceViewPreferences, WorkspaceSnapshot,
};
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

#[tauri::command]
fn search_skills(
    query: SkillQuery,
    state: tauri::State<'_, AppState>,
) -> Result<SkillSearchResult, String> {
    state
        .workspace
        .search_skills(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn load_view_preferences(
    state: tauri::State<'_, AppState>,
) -> Result<SkillWorkspaceViewPreferences, String> {
    state
        .workspace
        .load_view_preferences()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_view_preferences(
    preferences: SkillWorkspaceViewPreferences,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .workspace
        .save_view_preferences(&preferences)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn skill_detail(
    instance_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SkillDetail, String> {
    state
        .workspace
        .skill_detail(&instance_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_skill_file(
    instance_id: String,
    relative_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<SkillFilePreview, String> {
    state
        .workspace
        .read_skill_file(&instance_id, &relative_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn validate_skill_draft(
    draft: SkillDraft,
    state: tauri::State<'_, AppState>,
) -> SkillDraftValidation {
    state.workspace.validate_skill_draft(&draft)
}

#[tauri::command]
fn plan_skill_change(
    draft: SkillDraft,
    state: tauri::State<'_, AppState>,
) -> Result<SkillChangePlan, String> {
    state
        .workspace
        .plan_skill_change(&draft)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn execute_skill_change(
    plan_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<SkillChangeOutcome, String> {
    state
        .workspace
        .execute_skill_change(plan_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn undo_skill_change(
    operation_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<SkillChangeOutcome, String> {
    state
        .workspace
        .undo_skill_change(operation_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn latest_undoable_skill_change(
    state: tauri::State<'_, AppState>,
) -> Result<Option<SkillChangeRecord>, String> {
    state
        .workspace
        .latest_undoable_skill_change()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn review_duplicate_groups(state: tauri::State<'_, AppState>) -> Result<DuplicateReview, String> {
    state
        .workspace
        .review_duplicate_groups()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_duplicate_decision(
    instance_ids: Vec<String>,
    kind: DuplicateDecisionKind,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .workspace
        .save_duplicate_decision(&instance_ids, kind)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn duplicate_decisions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DuplicateDecisionRecord>, String> {
    state
        .workspace
        .duplicate_decisions()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_duplicate_decision(
    decision_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .workspace
        .restore_duplicate_decision(decision_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn skill_organization(
    state: tauri::State<'_, AppState>,
) -> Result<SkillOrganizationSnapshot, String> {
    state
        .workspace
        .skill_organization()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_skill_group(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<SkillOrganizationSnapshot, String> {
    state
        .workspace
        .create_skill_group(&name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_skill_group(
    group_id: i64,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<SkillOrganizationSnapshot, String> {
    state
        .workspace
        .rename_skill_group(group_id, &name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_skill_group(
    group_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<SkillOrganizationSnapshot, String> {
    state
        .workspace
        .delete_skill_group(group_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_skill_organization_change(
    change: SkillOrganizationChange,
    state: tauri::State<'_, AppState>,
) -> Result<SkillOrganizationSnapshot, String> {
    state
        .workspace
        .apply_skill_organization_change(&change)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn reorder_skill_group(
    group_id: i64,
    ordered_instance_ids: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<SkillOrganizationSnapshot, String> {
    state
        .workspace
        .reorder_skill_group(group_id, &ordered_instance_ids)
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
            remove_skill_root,
            search_skills,
            load_view_preferences,
            save_view_preferences,
            skill_detail,
            read_skill_file,
            validate_skill_draft,
            plan_skill_change,
            execute_skill_change,
            undo_skill_change,
            latest_undoable_skill_change,
            review_duplicate_groups,
            save_duplicate_decision,
            duplicate_decisions,
            restore_duplicate_decision,
            skill_organization,
            create_skill_group,
            rename_skill_group,
            delete_skill_group,
            apply_skill_organization_change,
            reorder_skill_group
        ])
        .run(tauri::generate_context!())
        .expect("启动 Skill 管理器失败");
}
