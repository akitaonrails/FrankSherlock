mod config;
mod db;
mod error;
mod models;
mod query_parser;
mod scan;

use std::process::Command;

use config::{prepare_dirs, resolve_paths, AppPaths};
use error::{AppError, AppResult};
use models::{CleanupResult, HealthStatus, ScanSummary, SearchRequest, SearchResponse};
use tauri::Manager;
use tauri::State;

#[derive(Clone)]
struct AppState {
    paths: AppPaths,
}

#[tauri::command]
fn app_health() -> HealthStatus {
    HealthStatus {
        status: "ok".to_string(),
        mode: "local-only".to_string(),
    }
}

#[tauri::command]
fn get_app_paths(state: State<'_, AppState>) -> Result<config::AppPathsView, String> {
    Ok(state.paths.view())
}

#[tauri::command]
fn ensure_database(state: State<'_, AppState>) -> Result<models::DbStats, String> {
    db::init_database(&state.paths.db_file)
        .and_then(|_| db::database_stats(&state.paths.db_file))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn parse_query_nl(query: String) -> models::ParsedQuery {
    query_parser::parse_query(&query)
}

#[tauri::command]
async fn search_images(
    request: SearchRequest,
    state: State<'_, AppState>,
) -> Result<SearchResponse, String> {
    let db_path = state.paths.db_file.clone();
    tauri::async_runtime::spawn_blocking(move || db::search_images(&db_path, &request))
        .await
        .map_err(|e| AppError::Join(e.to_string()).to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn scan_root(root_path: String, state: State<'_, AppState>) -> Result<ScanSummary, String> {
    let db_path = state.paths.db_file.clone();
    tauri::async_runtime::spawn_blocking(move || scan::scan_root_and_sync(&db_path, &root_path))
        .await
        .map_err(|e| AppError::Join(e.to_string()).to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn cleanup_ollama_models() -> Result<CleanupResult, String> {
    cleanup_ollama_models_impl().map_err(|e| e.to_string())
}

fn cleanup_ollama_models_impl() -> AppResult<CleanupResult> {
    let output = Command::new("ollama").arg("ps").output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut models = Vec::new();
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(model) = line.split_whitespace().next() {
            models.push(model.to_string());
        }
    }

    let mut stopped = 0_u64;
    for model in &models {
        let status = Command::new("ollama").args(["stop", model]).status()?;
        if status.success() {
            stopped += 1;
        }
    }

    Ok(CleanupResult {
        running_models: models.len() as u64,
        stopped_models: stopped,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = resolve_paths()
        .and_then(|paths| {
            prepare_dirs(&paths)?;
            db::init_database(&paths.db_file)?;
            Ok(paths)
        })
        .expect("failed to initialize application paths/database");

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(AppState { paths })
        .setup(|app| {
            if let Some(root_path) = std::env::args().nth(1) {
                let state = app.state::<AppState>();
                let db_path = state.paths.db_file.clone();
                tauri::async_runtime::spawn(async move {
                    let root = root_path.clone();
                    let _ = tauri::async_runtime::spawn_blocking(move || {
                        scan::scan_root_and_sync(&db_path, &root)
                    })
                    .await;
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            get_app_paths,
            ensure_database,
            parse_query_nl,
            search_images,
            scan_root,
            cleanup_ollama_models
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn cleanup_handles_empty_ps_output() {
        // This test only validates parser behavior for ps-like output.
        let mock = "NAME\tID\tSIZE\tPROCESSOR\tUNTIL\n";
        let models: Vec<String> = mock
            .lines()
            .skip(1)
            .filter_map(|line| line.split_whitespace().next().map(ToString::to_string))
            .collect();
        assert!(models.is_empty());
    }
}
