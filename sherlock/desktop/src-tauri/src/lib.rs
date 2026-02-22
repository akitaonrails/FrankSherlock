mod classify;
mod config;
mod db;
mod error;
mod models;
mod query_parser;
mod runtime;
mod scan;
mod thumbnail;

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use config::{prepare_dirs, resolve_paths, AppPaths};
use error::{AppError, AppResult};
use models::{
    CleanupResult, HealthStatus, RuntimeStatus, ScanJobStatus, SearchRequest, SearchResponse,
    SetupDownloadStatus, SetupStatus,
};
use tauri::Manager;
use tauri::State;

const REQUIRED_OLLAMA_MODELS: &[&str] = &["qwen2.5vl:7b"];

#[derive(Clone, Debug)]
struct SetupDownloadState {
    status: String,
    model: Option<String>,
    progress_pct: f32,
    message: String,
}

impl SetupDownloadState {
    fn idle() -> Self {
        Self {
            status: "idle".to_string(),
            model: None,
            progress_pct: 0.0,
            message: "No download in progress".to_string(),
        }
    }

    fn as_view(&self) -> SetupDownloadStatus {
        SetupDownloadStatus {
            status: self.status.clone(),
            model: self.model.clone(),
            progress_pct: self.progress_pct,
            message: self.message.clone(),
        }
    }
}

#[derive(Clone)]
struct AppState {
    paths: AppPaths,
    running_scan_jobs: Arc<Mutex<HashSet<i64>>>,
    setup_download: Arc<Mutex<SetupDownloadState>>,
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
fn get_setup_status(state: State<'_, AppState>) -> SetupStatus {
    compute_setup_status(state.inner())
}

#[tauri::command]
fn start_setup_download(state: State<'_, AppState>) -> Result<SetupDownloadStatus, String> {
    let setup = compute_setup_status(state.inner());
    if !setup.ollama_available {
        return Err("Ollama is not active. Start it first (`ollama serve`).".to_string());
    }
    if setup.missing_models.is_empty() {
        return Ok(setup.download);
    }

    {
        let current = state
            .setup_download
            .lock()
            .expect("setup download mutex poisoned");
        if current.status == "running" {
            return Ok(current.as_view());
        }
    }

    let model = setup
        .missing_models
        .first()
        .cloned()
        .ok_or_else(|| "No missing model to download".to_string())?;

    {
        let mut current = state
            .setup_download
            .lock()
            .expect("setup download mutex poisoned");
        current.status = "running".to_string();
        current.model = Some(model.clone());
        current.progress_pct = 0.0;
        current.message = format!("Starting download for {model}");
    }

    let setup_state = state.setup_download.clone();
    tauri::async_runtime::spawn(async move {
        run_model_download(setup_state, model).await;
    });

    Ok(state
        .setup_download
        .lock()
        .expect("setup download mutex poisoned")
        .as_view())
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
fn start_scan(root_path: String, state: State<'_, AppState>) -> Result<ScanJobStatus, String> {
    let setup = compute_setup_status(state.inner());
    if !setup.is_ready {
        return Err(
            "Setup incomplete: ensure Ollama is running and required models are installed."
                .to_string(),
        );
    }

    let job = scan::start_or_resume_scan_job(&state.paths.db_file, &root_path)
        .map_err(|e| e.to_string())?;
    let app_state = state.inner().clone();
    spawn_scan_worker_if_needed(app_state, job.id);
    db::get_scan_job(&state.paths.db_file, job.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "scan job not found after start".to_string())
}

#[tauri::command]
fn get_scan_job(job_id: i64, state: State<'_, AppState>) -> Result<Option<ScanJobStatus>, String> {
    db::get_scan_job(&state.paths.db_file, job_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_active_scans(state: State<'_, AppState>) -> Result<Vec<ScanJobStatus>, String> {
    db::list_resumable_scan_jobs(&state.paths.db_file).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_runtime_status() -> RuntimeStatus {
    runtime::gather_runtime_status()
}

#[tauri::command]
fn cleanup_ollama_models() -> Result<CleanupResult, String> {
    cleanup_ollama_models_impl().map_err(|e| e.to_string())
}

fn compute_setup_status(app_state: &AppState) -> SetupStatus {
    let required_models = REQUIRED_OLLAMA_MODELS
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>();

    let installed = runtime::list_installed_ollama_models();
    let ollama_available = installed.is_some();
    let missing_models = if let Some(models) = installed {
        required_models
            .iter()
            .filter(|required| !models.iter().any(|m| m == *required))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        required_models.clone()
    };

    let instructions = if !ollama_available {
        vec![
            "Start Ollama service first.".to_string(),
            "Terminal option: run `ollama serve`".to_string(),
            "Then click 'Recheck setup' in the app.".to_string(),
        ]
    } else if !missing_models.is_empty() {
        vec![
            format!("Download required model(s): {}", missing_models.join(", ")),
            "Use the 'Download required model' button and wait for completion.".to_string(),
        ]
    } else {
        vec!["Setup complete.".to_string()]
    };

    let download = app_state
        .setup_download
        .lock()
        .expect("setup download mutex poisoned")
        .as_view();

    SetupStatus {
        is_ready: ollama_available && missing_models.is_empty() && download.status != "running",
        ollama_available,
        required_models,
        missing_models,
        instructions,
        download,
    }
}

async fn run_model_download(setup_state: Arc<Mutex<SetupDownloadState>>, model: String) {
    let child = Command::new("ollama")
        .args(["pull", &model])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let Ok(mut child) = child else {
        let mut state = setup_state.lock().expect("setup download mutex poisoned");
        state.status = "failed".to_string();
        state.message = "Could not spawn `ollama pull` process.".to_string();
        return;
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(out) = stdout {
        let reader = BufReader::new(out);
        for line in reader.lines().map_while(Result::ok) {
            update_download_state_from_line(&setup_state, &model, &line);
        }
    }
    if let Some(err) = stderr {
        let reader = BufReader::new(err);
        for line in reader.lines().map_while(Result::ok) {
            update_download_state_from_line(&setup_state, &model, &line);
        }
    }

    match child.wait() {
        Ok(status) if status.success() => {
            let mut state = setup_state.lock().expect("setup download mutex poisoned");
            state.status = "completed".to_string();
            state.progress_pct = 100.0;
            state.message = format!("Model {model} downloaded.");
        }
        Ok(status) => {
            let mut state = setup_state.lock().expect("setup download mutex poisoned");
            state.status = "failed".to_string();
            state.message = format!("Model download failed with code {:?}", status.code());
        }
        Err(err) => {
            let mut state = setup_state.lock().expect("setup download mutex poisoned");
            state.status = "failed".to_string();
            state.message = format!("Failed to wait for pull process: {err}");
        }
    }
}

fn update_download_state_from_line(
    setup_state: &Arc<Mutex<SetupDownloadState>>,
    model: &str,
    line: &str,
) {
    let mut state = setup_state.lock().expect("setup download mutex poisoned");
    state.model = Some(model.to_string());
    state.status = "running".to_string();
    if let Some(progress) = parse_progress_percent(line) {
        state.progress_pct = progress;
    }
    state.message = line.trim().to_string();
}

fn parse_progress_percent(line: &str) -> Option<f32> {
    let percent_pos = line.find('%')?;
    let prefix = &line[..percent_pos];
    let start = prefix
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let number = prefix.get(start..)?.trim();
    number.parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0))
}

fn build_scan_context(paths: &AppPaths) -> models::ScanContext {
    let surya_script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("surya_ocr.py");
    models::ScanContext {
        db_path: paths.db_file.clone(),
        thumbnails_dir: paths.thumbnails_dir.clone(),
        tmp_dir: paths.tmp_dir.clone(),
        surya_venv_dir: paths.surya_venv_dir.clone(),
        surya_script,
        model: REQUIRED_OLLAMA_MODELS[0].to_string(),
    }
}

fn spawn_scan_worker_if_needed(app_state: AppState, job_id: i64) {
    {
        let mut guard = app_state
            .running_scan_jobs
            .lock()
            .expect("scan job mutex poisoned");
        if guard.contains(&job_id) {
            return;
        }
        guard.insert(job_id);
    }

    let scan_ctx = build_scan_context(&app_state.paths);
    let jobs = app_state.running_scan_jobs.clone();
    let app_state_for_task = app_state.clone();
    tauri::async_runtime::spawn(async move {
        let result =
            tauri::async_runtime::spawn_blocking(move || scan::run_scan_job(&scan_ctx, job_id))
                .await
                .map_err(|e| AppError::Join(e.to_string()))
                .and_then(|v| v);

        if let Err(err) = result {
            let _ = db::fail_scan_job(
                &app_state_for_task.paths.db_file,
                job_id,
                &format!("scan failed: {err}"),
            );
        }

        let mut guard = jobs.lock().expect("scan job mutex poisoned");
        guard.remove(&job_id);
    });
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
            db::recover_incomplete_scan_jobs(&paths.db_file)?;
            Ok(paths)
        })
        .expect("failed to initialize application paths/database");

    let app_state = AppState {
        paths,
        running_scan_jobs: Arc::new(Mutex::new(HashSet::new())),
        setup_download: Arc::new(Mutex::new(SetupDownloadState::idle())),
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(app_state.clone())
        .setup(move |app| {
            for job in db::list_resumable_scan_jobs(&app_state.paths.db_file)? {
                spawn_scan_worker_if_needed(app_state.clone(), job.id);
            }
            if let Some(root_path) = std::env::args().nth(1) {
                let state = app.state::<AppState>();
                if let Ok(job) = scan::start_or_resume_scan_job(&state.paths.db_file, &root_path) {
                    spawn_scan_worker_if_needed(state.inner().clone(), job.id);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            get_app_paths,
            ensure_database,
            parse_query_nl,
            get_setup_status,
            start_setup_download,
            search_images,
            start_scan,
            get_scan_job,
            list_active_scans,
            get_runtime_status,
            cleanup_ollama_models
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_handles_empty_ps_output() {
        let mock = "NAME\tID\tSIZE\tPROCESSOR\tUNTIL\n";
        let models: Vec<String> = mock
            .lines()
            .skip(1)
            .filter_map(|line| line.split_whitespace().next().map(ToString::to_string))
            .collect();
        assert!(models.is_empty());
    }

    #[test]
    fn extracts_progress_percent() {
        assert_eq!(parse_progress_percent("pulling ... 34%"), Some(34.0));
        assert_eq!(parse_progress_percent("12.5% complete"), Some(12.5));
        assert_eq!(parse_progress_percent("done"), None);
    }
}
