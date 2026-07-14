//! Skill 管理器桌面壳。

use std::path::PathBuf;

use skill_workspace::{
    DuplicateDecisionKind, DuplicateDecisionRecord, DuplicateReview, FileOperationBatchOutcome,
    FileOperationPlan, FileOperationRecord, FileOperationRequest, SkillChangeOutcome,
    SkillChangePlan, SkillChangeRecord, SkillDetail, SkillDraft, SkillDraftValidation,
    SkillFilePreview, SkillOrganizationChange, SkillOrganizationSnapshot, SkillQuery,
    SkillSearchResult, SkillWorkspace, SkillWorkspaceViewPreferences, WorkspaceError,
    WorkspaceSnapshot, ZipImportRequest,
};
use tauri::Manager;

#[cfg(feature = "desktop-smoke")]
mod desktop_smoke;

struct AppState {
    workspace: SkillWorkspace,
}

async fn run_workspace_task<T>(
    task: impl FnOnce() -> Result<T, WorkspaceError> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|_| "本地任务意外中断，请重试。".to_owned())?
        .map_err(|error| error.user_message())
}

#[tauri::command]
fn workspace_snapshot(state: tauri::State<'_, AppState>) -> Result<WorkspaceSnapshot, String> {
    state
        .workspace
        .snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn authorize_skill_root(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = state.workspace.clone();
    run_workspace_task(move || workspace.add_root(PathBuf::from(path))).await
}

#[tauri::command]
async fn rescan_skill_root(
    root_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = state.workspace.clone();
    run_workspace_task(move || workspace.rescan_root(root_id)).await
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
async fn search_skills(
    query: SkillQuery,
    state: tauri::State<'_, AppState>,
) -> Result<SkillSearchResult, String> {
    let workspace = state.workspace.clone();
    run_workspace_task(move || workspace.search_skills(&query)).await
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
    state.workspace.plan_skill_change(&draft).map_err(|error| {
        #[cfg(feature = "desktop-smoke")]
        eprintln!("macOS 桌面冒烟变化计划失败：{error:?}");
        error.user_message()
    })
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
async fn review_duplicate_groups(
    state: tauri::State<'_, AppState>,
) -> Result<DuplicateReview, String> {
    let workspace = state.workspace.clone();
    run_workspace_task(move || workspace.review_duplicate_groups()).await
}

#[tauri::command]
async fn save_duplicate_decision(
    instance_ids: Vec<String>,
    kind: DuplicateDecisionKind,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let workspace = state.workspace.clone();
    run_workspace_task(move || workspace.save_duplicate_decision(&instance_ids, kind)).await
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
async fn restore_duplicate_decision(
    decision_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let workspace = state.workspace.clone();
    run_workspace_task(move || workspace.restore_duplicate_decision(decision_id)).await
}

#[tauri::command]
fn plan_duplicate_merge(
    master_instance_id: String,
    target_instance_ids: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<FileOperationPlan, String> {
    state
        .workspace
        .plan_duplicate_merge(&master_instance_id, &target_instance_ids)
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

#[tauri::command]
fn plan_file_operations(
    request: FileOperationRequest,
    state: tauri::State<'_, AppState>,
) -> Result<FileOperationPlan, String> {
    state
        .workspace
        .plan_file_operations(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_zip_import(
    request: ZipImportRequest,
    state: tauri::State<'_, AppState>,
) -> Result<FileOperationPlan, String> {
    state
        .workspace
        .preview_zip_import(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn execute_file_operation_plan(
    plan_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<FileOperationBatchOutcome, String> {
    state
        .workspace
        .execute_file_operation_plan(plan_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_file_operation_plan(
    plan_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .workspace
        .cancel_file_operation_plan(plan_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn file_operation_history(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileOperationRecord>, String> {
    state
        .workspace
        .file_operation_history()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn latest_undoable_file_operation(
    state: tauri::State<'_, AppState>,
) -> Result<Option<FileOperationRecord>, String> {
    state
        .workspace
        .latest_undoable_file_operation()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn undo_file_operation_batch(
    batch_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceSnapshot, String> {
    state
        .workspace
        .undo_file_operation_batch(batch_id)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(not(feature = "desktop-smoke"))]
            let database_path = {
                let app_data_dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(&app_data_dir)?;
                app_data_dir.join("skill-management.sqlite3")
            };
            #[cfg(feature = "desktop-smoke")]
            let database_path = desktop_smoke::database_path()?;
            let workspace = SkillWorkspace::open(database_path)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            #[cfg(feature = "desktop-smoke")]
            desktop_smoke::monitor(app.handle().clone(), workspace.clone());
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
            plan_duplicate_merge,
            skill_organization,
            create_skill_group,
            rename_skill_group,
            delete_skill_group,
            apply_skill_organization_change,
            reorder_skill_group,
            plan_file_operations,
            preview_zip_import,
            execute_file_operation_plan,
            cancel_file_operation_plan,
            file_operation_history,
            latest_undoable_file_operation,
            undo_file_operation_batch
        ]);
    #[cfg(feature = "desktop-smoke")]
    let builder = {
        let smoke_script = desktop_smoke::script();
        builder.on_page_load(move |webview, payload| {
            eprintln!(
                "macOS 桌面冒烟页面事件：{:?} {}",
                payload.event(),
                payload.url()
            );
            if payload.event() == tauri::webview::PageLoadEvent::Finished
                && let Err(error) = webview.eval(&smoke_script)
            {
                eprintln!("macOS 桌面冒烟脚本注入失败：{error}");
            }
        })
    };
    builder
        .run(tauri::generate_context!())
        .expect("启动 Skill 管理器失败");
}
