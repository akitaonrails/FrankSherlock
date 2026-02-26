mod classify;
mod config;
mod db;
mod error;
mod exif;
mod llm;
mod models;
mod pdf;
mod platform;
mod query_parser;
mod runtime;
mod scan;
mod similarity;
mod thumbnail;
mod video;
mod video_server;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use config::{prepare_dirs, resolve_paths, AppPaths};
use error::AppError;
use models::{
    Album, DeleteFilesResult, DuplicatesResponse, HealthCheckOutcome, HealthStatus, PdfPassword,
    ProtectedPdfInfo, PurgeResult, RenameFileResult, RetryProtectedPdfsResult, RootInfo,
    RuntimeStatus, ScanJobStatus, SearchRequest, SearchResponse, SetupDownloadStatus, SetupStatus,
    SmartFolder, SubdirEntry, VenvProvisionStatus,
};
use tauri::Manager;
use tauri::State;

#[derive(Clone)]
struct AppState {
    paths: AppPaths,
    read_only: bool,
    gpu_info: platform::gpu::GpuInfo,
    running_scan_jobs: Arc<Mutex<HashSet<i64>>>,
    setup_download: Arc<Mutex<llm::DownloadState>>,
    venv_provision: Arc<Mutex<platform::python::VenvProvisionState>>,
    cached_system_python: Option<std::path::PathBuf>,
    cancel_flags: Arc<Mutex<HashMap<i64, Arc<AtomicBool>>>>,
    cli_folder_path: Option<String>,
}

#[tauri::command]
fn app_health(state: State<'_, AppState>) -> HealthStatus {
    HealthStatus {
        status: "ok".to_string(),
        mode: if state.read_only {
            "read-only".to_string()
        } else {
            "local-only".to_string()
        },
        read_only: state.read_only,
    }
}

fn require_writable(state: &AppState) -> Result<(), String> {
    if state.read_only {
        Err("Database is read-only".to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
fn get_app_paths(state: State<'_, AppState>) -> Result<config::AppPathsView, String> {
    Ok(state.paths.view())
}

#[tauri::command]
fn get_cli_folder_path(state: State<'_, AppState>) -> Option<String> {
    state.cli_folder_path.clone()
}

#[tauri::command]
fn ensure_database(state: State<'_, AppState>) -> Result<models::DbStats, String> {
    let mut stats = if state.read_only {
        db::database_stats(&state.paths.db_file).map_err(|e| e.to_string())?
    } else {
        db::init_database(&state.paths.db_file)
            .and_then(|_| db::database_stats(&state.paths.db_file))
            .map_err(|e| e.to_string())?
    };
    stats.db_size_bytes = file_size_bytes(&state.paths.db_file);
    stats.thumbs_size_bytes = dir_size_bytes(&state.paths.thumbnails_dir);
    Ok(stats)
}

fn file_size_bytes(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn dir_size_bytes(path: &std::path::Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
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
        llm::management::run_model_download(setup_state, model).await;
    });

    Ok(state
        .setup_download
        .lock()
        .expect("setup download mutex poisoned")
        .as_view())
}

#[tauri::command]
fn start_venv_provision(state: State<'_, AppState>) -> Result<VenvProvisionStatus, String> {
    // If already running, return current status
    {
        let current = state
            .venv_provision
            .lock()
            .expect("venv provision mutex poisoned");
        if current.status == "running" {
            return Ok(current.as_view());
        }
    }

    let system_python = state
        .cached_system_python
        .clone()
        .ok_or_else(|| "No Python 3 found on this system. Install Python 3 first.".to_string())?;

    // Check if venv already works
    let python_status = platform::python::check_python_available(&state.paths.surya_venv_dir);
    if python_status.available {
        let venv_python = platform::python::python_venv_binary(&state.paths.surya_venv_dir);
        let surya_ok = std::process::Command::new(&venv_python)
            .args(["-c", "import surya"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if surya_ok {
            let mut current = state
                .venv_provision
                .lock()
                .expect("venv provision mutex poisoned");
            current.status = "completed".to_string();
            current.progress_pct = 100.0;
            current.message = "Surya OCR is already installed.".to_string();
            return Ok(current.as_view());
        }
    }

    // Set running state
    {
        let mut current = state
            .venv_provision
            .lock()
            .expect("venv provision mutex poisoned");
        current.status = "running".to_string();
        current.step = "starting".to_string();
        current.progress_pct = 0.0;
        current.message = "Starting OCR setup...".to_string();
    }

    let provision_state = state.venv_provision.clone();
    let venv_dir = state.paths.surya_venv_dir.clone();
    tauri::async_runtime::spawn(async move {
        platform::python::run_venv_provision(provision_state, system_python, venv_dir).await;
    });

    Ok(state
        .venv_provision
        .lock()
        .expect("venv provision mutex poisoned")
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
fn start_scan(
    root_path: String,
    skip_classify: Option<bool>,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<ScanJobStatus, String> {
    require_writable(state.inner())?;
    let skip = skip_classify.unwrap_or(false);

    // Only require Ollama setup for full scans (not metadata-only refreshes)
    if !skip {
        let setup = compute_setup_status(state.inner());
        if !setup.is_ready {
            return Err(
                "Setup incomplete: ensure Ollama is running and required models are installed."
                    .to_string(),
            );
        }
    }

    let job = scan::start_or_resume_scan_job(&state.paths.db_file, &root_path)
        .map_err(|e| e.to_string())?;
    // If this root is a child of an existing parent root, adopt files from parent
    let _ = db::adopt_child_files(&state.paths.db_file, job.root_id, &job.root_path);
    let app_state = state.inner().clone();
    spawn_scan_worker_if_needed(app_state, &app_handle, job.id, skip);
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
fn cancel_scan(job_id: i64, state: State<'_, AppState>) -> Result<bool, String> {
    require_writable(state.inner())?;
    let flags = state
        .cancel_flags
        .lock()
        .expect("cancel flags mutex poisoned");
    if let Some(flag) = flags.get(&job_id) {
        flag.store(true, Ordering::Relaxed);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn remove_root(root_id: i64, state: State<'_, AppState>) -> Result<PurgeResult, String> {
    require_writable(state.inner())?;
    // If this root has a parent root, reassign files back instead of deleting
    if let Some(_parent_id) =
        db::reassign_to_parent_root(&state.paths.db_file, root_id).map_err(|e| e.to_string())?
    {
        // Files reassigned to parent, root + jobs already deleted by reassign_to_parent_root
        return Ok(PurgeResult {
            files_removed: 0,
            jobs_removed: 0,
            thumbs_cleaned: 0,
        });
    }
    db::purge_root(&state.paths.db_file, root_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_roots(state: State<'_, AppState>) -> Result<Vec<RootInfo>, String> {
    db::list_roots(&state.paths.db_file).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_subdirectories(
    root_id: i64,
    parent_prefix: String,
    state: State<'_, AppState>,
) -> Result<Vec<SubdirEntry>, String> {
    db::list_subdirectories(&state.paths.db_file, root_id, &parent_prefix)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn copy_files_to_clipboard(paths: Vec<String>) -> Result<(), String> {
    let text = paths.join("\n");
    platform::clipboard::copy_to_clipboard(&text)
}

#[tauri::command]
fn delete_files(
    file_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<DeleteFilesResult, String> {
    require_writable(state.inner())?;

    // Phase 1: collect file info from DB (read-only, no mutation)
    let file_infos =
        db::collect_files_for_delete(&state.paths.db_file, &file_ids).map_err(|e| e.to_string())?;

    // Phase 2: delete from filesystem first
    let mut fs_deleted_ids = Vec::new();
    let mut deleted_count = 0u64;
    let mut errors = Vec::new();

    for (fid, abs_path, thumb_path) in &file_infos {
        match std::fs::remove_file(abs_path) {
            Ok(()) => {
                fs_deleted_ids.push(*fid);
                deleted_count += 1;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File already gone from disk — safe to remove DB record too
                fs_deleted_ids.push(*fid);
                deleted_count += 1;
            }
            Err(e) => {
                // Filesystem delete failed — do NOT remove from DB, keep state consistent
                errors.push(format!("{abs_path}: {e}"));
            }
        }
        if let Some(tp) = thumb_path {
            let _ = std::fs::remove_file(tp);
        }
    }

    // Phase 3: only remove DB records for files successfully deleted from disk
    if let Err(e) = db::delete_file_records(&state.paths.db_file, &fs_deleted_ids) {
        errors.push(format!("DB cleanup error: {e}"));
    }

    Ok(DeleteFilesResult {
        deleted_count,
        errors,
    })
}

#[tauri::command]
fn rename_file(
    file_id: i64,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<RenameFileResult, String> {
    require_writable(state.inner())?;

    // Validate new_name
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("Filename cannot be empty".to_string());
    }
    if new_name.contains('/') || new_name.contains('\\') {
        return Err("Filename cannot contain path separators".to_string());
    }

    let (old_abs, old_rel, _thumb) =
        db::get_file_path_info(&state.paths.db_file, file_id).map_err(|e| e.to_string())?;

    // Compute new paths by replacing the filename component
    let old_abs_path = std::path::Path::new(&old_abs);
    let new_abs_path = old_abs_path
        .parent()
        .map(|p| p.join(&new_name))
        .ok_or_else(|| "Cannot determine parent directory".to_string())?;
    let new_abs_str = new_abs_path.to_string_lossy().to_string();

    let new_rel = if let Some(parent) = platform::paths::rel_path_parent(&old_rel) {
        format!("{parent}/{new_name}")
    } else {
        new_name.clone()
    };

    // Phase 1: rename on disk first
    std::fs::rename(&old_abs, &new_abs_path).map_err(|e| format!("Rename failed: {e}"))?;

    // Phase 2: update DB — rollback filesystem rename on failure
    if let Err(e) = db::rename_file_record(
        &state.paths.db_file,
        file_id,
        &new_rel,
        &new_abs_str,
        &new_name,
    ) {
        // Attempt to restore the original filename on disk
        if let Err(rollback_err) = std::fs::rename(&new_abs_path, &old_abs) {
            log::error!(
                "DB update failed AND filesystem rollback failed: db_err={e}, rollback_err={rollback_err}, \
                 file is now at {new_abs_str} but DB still says {old_abs}"
            );
            return Err(format!(
                "Rename partially failed: DB error ({e}) and could not restore original filename ({rollback_err})"
            ));
        }
        return Err(format!(
            "Rename failed (DB update error, filesystem restored): {e}"
        ));
    }

    Ok(RenameFileResult {
        file_id,
        new_rel_path: new_rel,
        new_abs_path: new_abs_str,
        new_filename: new_name,
    })
}

#[tauri::command]
fn get_file_metadata(
    file_id: i64,
    state: State<'_, AppState>,
) -> Result<models::FileMetadata, String> {
    db::get_file_metadata(&state.paths.db_file, file_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_file_properties(
    file_id: i64,
    state: State<'_, AppState>,
) -> Result<models::FileProperties, String> {
    let mut props =
        db::get_file_properties(&state.paths.db_file, file_id).map_err(|e| e.to_string())?;

    // Enrich with EXIF data from the actual file (skip for videos)
    let path = std::path::Path::new(&props.abs_path);
    let is_vid = video::is_video_file(path);
    if path.exists() && !is_vid {
        props.exif = exif::extract_exif_details(path);

        // Get image dimensions (fast header-only read) if EXIF didn't have them
        if props.exif.image_width.is_none() || props.exif.image_height.is_none() {
            if let Ok(dim) = image::image_dimensions(path) {
                props.exif.image_width = Some(dim.0);
                props.exif.image_height = Some(dim.1);
            }
        }
    }

    Ok(props)
}

#[tauri::command]
fn update_file_metadata(
    file_id: i64,
    media_type: String,
    description: String,
    extracted_text: String,
    canonical_mentions: String,
    location_text: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    require_writable(state.inner())?;
    db::update_file_metadata(
        &state.paths.db_file,
        file_id,
        &media_type,
        &description,
        &extracted_text,
        &canonical_mentions,
        &location_text,
    )
    .map_err(|e| e.to_string())
}

// ── Duplicates ──────────────────────────────────────────────────────

#[tauri::command]
async fn find_duplicates(
    root_scope: Vec<i64>,
    near_threshold: Option<f32>,
    state: State<'_, AppState>,
) -> Result<DuplicatesResponse, String> {
    let db_path = state.paths.db_file.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db::find_duplicates(&db_path, &root_scope, near_threshold)
    })
    .await
    .map_err(|e| AppError::Join(e.to_string()).to_string())?
    .map_err(|e| e.to_string())
}

// ── Album commands ───────────────────────────────────────────────────

#[tauri::command]
fn create_album(name: String, state: State<'_, AppState>) -> Result<Album, String> {
    require_writable(state.inner())?;
    db::create_album(&state.paths.db_file, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_album(album_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    require_writable(state.inner())?;
    db::delete_album(&state.paths.db_file, album_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    db::list_albums(&state.paths.db_file).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_files_to_album(
    album_id: i64,
    file_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    require_writable(state.inner())?;
    db::add_files_to_album(&state.paths.db_file, album_id, &file_ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_files_from_album(
    album_id: i64,
    file_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    require_writable(state.inner())?;
    db::remove_files_from_album(&state.paths.db_file, album_id, &file_ids)
        .map_err(|e| e.to_string())
}

// ── Smart folder commands ───────────────────────────────────────────

#[tauri::command]
fn create_smart_folder(
    name: String,
    query: String,
    state: State<'_, AppState>,
) -> Result<SmartFolder, String> {
    require_writable(state.inner())?;
    db::create_smart_folder(&state.paths.db_file, &name, &query).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_smart_folder(folder_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    require_writable(state.inner())?;
    db::delete_smart_folder(&state.paths.db_file, folder_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_smart_folders(state: State<'_, AppState>) -> Result<Vec<SmartFolder>, String> {
    db::list_smart_folders(&state.paths.db_file).map_err(|e| e.to_string())
}

// ── Reorder commands ────────────────────────────────────────────────

#[tauri::command]
fn reorder_roots(ids: Vec<i64>, state: State<'_, AppState>) -> Result<(), String> {
    require_writable(state.inner())?;
    db::reorder_roots(&state.paths.db_file, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_albums(ids: Vec<i64>, state: State<'_, AppState>) -> Result<(), String> {
    require_writable(state.inner())?;
    db::reorder_albums(&state.paths.db_file, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_smart_folders(ids: Vec<i64>, state: State<'_, AppState>) -> Result<(), String> {
    require_writable(state.inner())?;
    db::reorder_smart_folders(&state.paths.db_file, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_user_config() -> Result<serde_json::Value, String> {
    config::load_user_config().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_user_config(config: serde_json::Value, state: State<'_, AppState>) -> Result<(), String> {
    require_writable(state.inner())?;
    config::save_user_config(&config).map_err(|e| e.to_string())
}

// ── PDF Password commands ────────────────────────────────────────────

#[tauri::command]
fn add_pdf_password(
    password: String,
    label: String,
    state: State<'_, AppState>,
) -> Result<PdfPassword, String> {
    require_writable(state.inner())?;
    db::add_pdf_password(&state.paths.db_file, &password, &label).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_pdf_password(password_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    require_writable(state.inner())?;
    db::delete_pdf_password(&state.paths.db_file, password_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_pdf_passwords(state: State<'_, AppState>) -> Result<Vec<PdfPassword>, String> {
    db::list_pdf_passwords(&state.paths.db_file).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_protected_pdfs(state: State<'_, AppState>) -> Result<Vec<ProtectedPdfInfo>, String> {
    db::list_protected_pdfs(&state.paths.db_file).map_err(|e| e.to_string())
}

#[tauri::command]
async fn retry_protected_pdfs(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<RetryProtectedPdfsResult, String> {
    let db_path = state.paths.db_file.clone();
    let thumbnails_dir = state.paths.thumbnails_dir.clone();
    let scan_ctx = build_scan_context(state.inner(), &app_handle);

    tauri::async_runtime::spawn_blocking(move || {
        let protected = db::list_protected_pdfs(&db_path).map_err(|e| e.to_string())?;
        let passwords = db::get_all_pdf_password_strings(&db_path).map_err(|e| e.to_string())?;

        if passwords.is_empty() || protected.is_empty() {
            return Ok(RetryProtectedPdfsResult {
                total_attempted: protected.len() as u64,
                unlocked: 0,
                still_protected: protected.len() as u64,
            });
        }

        let total_attempted = protected.len() as u64;
        let mut unlocked = 0u64;

        for pdf in &protected {
            let abs = std::path::Path::new(&pdf.abs_path);
            if !abs.exists() {
                continue;
            }

            if let Some(pw) = crate::pdf::try_passwords(abs, &scan_ctx.pdfium_lib_path, &passwords)
            {
                // Re-classify with the working password
                let classification = classify::classify_pdf(
                    abs,
                    &scan_ctx.model,
                    &scan_ctx.tmp_dir,
                    &scan_ctx.surya_venv_dir,
                    &scan_ctx.surya_script,
                    &scan_ctx.pdfium_lib_path,
                    Some(&pw),
                );

                // Generate thumbnail
                let thumb_result = thumbnail::generate_pdf_thumbnail(
                    abs,
                    &thumbnails_dir,
                    &pdf.rel_path,
                    &scan_ctx.pdfium_lib_path,
                    Some(&pw),
                );

                // Update the DB record with actual classification
                let _ = db::update_file_classification(
                    &db_path,
                    pdf.id,
                    &classification.media_type,
                    &classification.description,
                    &classification.extracted_text,
                    &classification.canonical_mentions,
                    classification.confidence,
                    &classification.lang_hint,
                );

                if let Some(ref tr) = thumb_result {
                    let _ = db::update_file_thumb_path_by_id(&db_path, pdf.id, &tr.path);
                }

                unlocked += 1;
            }
        }

        Ok(RetryProtectedPdfsResult {
            total_attempted,
            unlocked,
            still_protected: total_attempted - unlocked,
        })
    })
    .await
    .map_err(|e| AppError::Join(e.to_string()).to_string())?
}

#[tauri::command]
async fn reclassify_pdf(
    file_id: i64,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    let db_path = state.paths.db_file.clone();
    let thumbnails_dir = state.paths.thumbnails_dir.clone();
    let scan_ctx = build_scan_context(state.inner(), &app_handle);

    tauri::async_runtime::spawn_blocking(move || {
        let (abs_path, rel_path, _) =
            db::get_file_path_info(&db_path, file_id).map_err(|e| e.to_string())?;
        let abs = std::path::Path::new(&abs_path);

        if !abs.exists() {
            return Ok(false);
        }

        let passwords = db::get_all_pdf_password_strings(&db_path).map_err(|e| e.to_string())?;
        let working_pw = crate::pdf::try_passwords(abs, &scan_ctx.pdfium_lib_path, &passwords);

        if working_pw.is_none() && crate::pdf::is_password_protected(abs, &scan_ctx.pdfium_lib_path)
        {
            return Ok(false);
        }

        let classification = classify::classify_pdf(
            abs,
            &scan_ctx.model,
            &scan_ctx.tmp_dir,
            &scan_ctx.surya_venv_dir,
            &scan_ctx.surya_script,
            &scan_ctx.pdfium_lib_path,
            working_pw.as_deref(),
        );

        let thumb_result = thumbnail::generate_pdf_thumbnail(
            abs,
            &thumbnails_dir,
            &rel_path,
            &scan_ctx.pdfium_lib_path,
            working_pw.as_deref(),
        );

        let _ = db::update_file_classification(
            &db_path,
            file_id,
            &classification.media_type,
            &classification.description,
            &classification.extracted_text,
            &classification.canonical_mentions,
            classification.confidence,
            &classification.lang_hint,
        );

        if let Some(ref tr) = thumb_result {
            let _ = db::update_file_thumb_path_by_id(&db_path, file_id, &tr.path);
        }

        Ok(true)
    })
    .await
    .map_err(|e| AppError::Join(e.to_string()).to_string())?
}

#[tauri::command]
fn get_video_stream_url(abs_path: String) -> String {
    video_server::stream_url(&abs_path)
}

fn compute_setup_status(app_state: &AppState) -> SetupStatus {
    let (model_tag, model_tier, model_reason) = llm::recommended_model(&app_state.gpu_info);
    let required_models = vec![model_tag.to_string()];

    let installed = llm::list_installed_models();
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

    let python_status = platform::python::check_python_available(&app_state.paths.surya_venv_dir);
    let os = platform::current_os();

    let mut instructions = if !ollama_available {
        let mut steps = vec!["Ollama is not detected.".to_string()];
        match os {
            platform::OsKind::Linux => {
                steps.push("Install: curl -fsSL https://ollama.com/install.sh | sh".to_string());
                steps.push("Start: ollama serve (or: systemctl start ollama)".to_string());
            }
            platform::OsKind::MacOS => {
                steps.push(
                    "Install: brew install ollama (or download from ollama.com/download)"
                        .to_string(),
                );
                steps.push(
                    "Start the Ollama app from Applications, or run: ollama serve".to_string(),
                );
            }
            platform::OsKind::Windows => {
                steps.push("Install: download from ollama.com/download".to_string());
                steps.push("Start Ollama from the Start Menu, or run: ollama serve".to_string());
            }
        }
        steps.push("Then click 'Recheck' above.".to_string());
        steps
    } else if !missing_models.is_empty() {
        vec![
            format!("Download required model(s): {}", missing_models.join(", ")),
            "Use the 'Download required model' button and wait for completion.".to_string(),
        ]
    } else {
        vec!["Setup complete.".to_string()]
    };

    // Surya/Python is a soft requirement (OCR enrichment)
    let system_python_found = app_state.cached_system_python.is_some();
    let surya_venv_ok = python_status.venv_exists && python_status.available;

    if !surya_venv_ok {
        if system_python_found {
            instructions
                .push("OCR (optional): Click 'Setup OCR' to auto-configure Surya OCR.".to_string());
        } else if !python_status.venv_exists || !python_status.available {
            match os {
                platform::OsKind::Linux => {
                    instructions.push(
                        "OCR (optional): Python 3 not found. Install Python 3 \
                         (e.g. sudo apt install python3 python3-venv) then relaunch."
                            .to_string(),
                    );
                }
                platform::OsKind::MacOS => {
                    instructions.push(
                        "OCR (optional): Python 3 not found. Install Python 3 \
                         (e.g. brew install python@3) then relaunch."
                            .to_string(),
                    );
                }
                platform::OsKind::Windows => {
                    instructions.push(
                        "OCR (optional): Python 3 not found. Install Python 3 \
                         from python.org/downloads, then relaunch."
                            .to_string(),
                    );
                }
            }
        }
    }

    let download = app_state
        .setup_download
        .lock()
        .expect("setup download mutex poisoned")
        .as_view();

    let venv_provision = app_state
        .venv_provision
        .lock()
        .expect("venv provision mutex poisoned")
        .as_view();

    let ffmpeg_available = video::is_ffmpeg_available();
    if !ffmpeg_available {
        instructions.push(
            "Video (optional): ffmpeg not found. Install ffmpeg for video thumbnails and classification.".to_string(),
        );
    }

    SetupStatus {
        is_ready: ollama_available && missing_models.is_empty() && download.status != "running",
        ollama_available,
        required_models,
        missing_models,
        instructions,
        download,
        python_available: python_status.available,
        python_version: python_status.version,
        surya_venv_ok,
        recommended_model: model_tag.to_string(),
        model_tier: model_tier.as_str().to_string(),
        model_selection_reason: model_reason,
        system_python_found,
        venv_provision,
        ffmpeg_available,
    }
}

fn resolve_surya_script(app_handle: &tauri::AppHandle) -> std::path::PathBuf {
    // In production builds, surya_ocr.py is bundled as a Tauri resource.
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let bundled = resource_dir.join("scripts").join("surya_ocr.py");
        if bundled.exists() {
            return bundled;
        }
    }
    // Dev fallback: use CARGO_MANIFEST_DIR (set at compile time)
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("surya_ocr.py")
}

fn resolve_pdfium_lib(app_handle: &tauri::AppHandle) -> std::path::PathBuf {
    let lib_name = platform::pdfium_lib_name();

    // macOS: Tauri places frameworks in Contents/Frameworks/ (ad-hoc signed).
    if cfg!(target_os = "macos") {
        if let Ok(resource_dir) = app_handle.path().resource_dir() {
            // resource_dir points to Contents/Resources/; Frameworks is a sibling.
            let frameworks_dir = resource_dir.parent().map(|p| p.join("Frameworks"));
            if let Some(fw) = frameworks_dir {
                if fw.join(lib_name).exists() {
                    return fw;
                }
            }
        }
    }

    // Linux/Windows: PDFium is bundled as a Tauri resource under lib/.
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let lib_dir = resource_dir.join("lib");
        if lib_dir.join(lib_name).exists() {
            return lib_dir;
        }
    }

    // Dev fallback: look in src-tauri/lib/
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib")
}

fn build_scan_context(app_state: &AppState, app_handle: &tauri::AppHandle) -> models::ScanContext {
    let surya_script = resolve_surya_script(app_handle);
    let pdfium_lib_path = resolve_pdfium_lib(app_handle);
    let (model_tag, _, _) = llm::recommended_model(&app_state.gpu_info);
    models::ScanContext {
        db_path: app_state.paths.db_file.clone(),
        thumbnails_dir: app_state.paths.thumbnails_dir.clone(),
        tmp_dir: app_state.paths.tmp_dir.clone(),
        surya_venv_dir: app_state.paths.surya_venv_dir.clone(),
        surya_script,
        model: model_tag.to_string(),
        pdfium_lib_path,
    }
}

fn spawn_scan_worker_if_needed(
    app_state: AppState,
    app_handle: &tauri::AppHandle,
    job_id: i64,
    skip_classify: bool,
) {
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

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut flags = app_state
            .cancel_flags
            .lock()
            .expect("cancel flags mutex poisoned");
        flags.insert(job_id, cancel_flag.clone());
    }

    let scan_ctx = build_scan_context(&app_state, app_handle);
    let jobs = app_state.running_scan_jobs.clone();
    let cancel_flags = app_state.cancel_flags.clone();
    let app_state_for_task = app_state.clone();
    tauri::async_runtime::spawn(async move {
        let flag = cancel_flag;
        let result = tauri::async_runtime::spawn_blocking(move || {
            scan::run_scan_job(&scan_ctx, job_id, Some(&flag), skip_classify)
        })
        .await
        .map_err(|e| AppError::Join(e.to_string()))
        .and_then(|v| v);

        if let Err(err) = &result {
            let _ = db::fail_scan_job(
                &app_state_for_task.paths.db_file,
                job_id,
                &format!("scan failed: {err}"),
            );
        }

        // Remove from tracking before checking if all jobs finished
        let mut guard = jobs.lock().expect("scan job mutex poisoned");
        guard.remove(&job_id);
        let all_jobs_done = guard.is_empty();
        drop(guard);

        let mut flags = cancel_flags.lock().expect("cancel flags mutex poisoned");
        flags.remove(&job_id);
        drop(flags);

        if result.is_ok() {
            // WAL checkpoint + backup after each successful scan
            if let Err(e) = db::wal_checkpoint(&app_state_for_task.paths.db_file) {
                log::warn!("WAL checkpoint after scan failed: {e}");
            }
            let backup_path = app_state_for_task.paths.db_dir.join("index.sqlite.bak");
            if let Err(e) = db::backup_database(&app_state_for_task.paths.db_file, &backup_path) {
                log::warn!("Post-scan backup failed: {e}");
            }
        }

        // Only unload models once ALL scan jobs have finished
        if all_jobs_done {
            if let Err(e) = llm::cleanup_loaded_models() {
                log::warn!("Auto-cleanup after all scans finished: {e}");
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (paths, read_only) = resolve_paths()
        .and_then(|paths| {
            let dirs_ok = prepare_dirs(&paths).is_ok();
            let init_ok = dirs_ok && db::init_database(&paths.db_file).is_ok();

            if init_ok {
                // Full read-write mode
                db::recover_incomplete_scan_jobs(&paths.db_file).ok();

                match db::startup_health_check(&paths.db_file, &paths.db_dir) {
                    Ok(HealthCheckOutcome::RestoredFromBackup) => {
                        log::warn!("Database restored from backup");
                    }
                    Ok(HealthCheckOutcome::Recreated) => {
                        log::warn!("Database was recreated (previous was corrupt)");
                    }
                    Ok(HealthCheckOutcome::Healthy) => {}
                    Err(e) => {
                        log::error!("Startup health check failed: {e}");
                    }
                }

                match db::validate_and_purge_stale_roots(&paths.db_file, &paths.thumbnails_dir) {
                    Ok(purged) => {
                        for root in &purged {
                            log::info!("Purged stale root: {root}");
                        }
                    }
                    Err(e) => {
                        log::warn!("Stale root validation failed: {e}");
                    }
                }

                Ok((paths, false))
            } else if paths.db_file.exists() {
                // DB exists but filesystem is not writable — read-only mode
                log::warn!("Running in read-only mode (filesystem not writable)");
                Ok((paths, true))
            } else {
                Err(AppError::Config(
                    "Cannot create database and no existing database found".into(),
                ))
            }
        })
        .expect("failed to initialize application paths/database");

    let gpu_info = platform::gpu::detect_gpu_memory();
    let cached_system_python = platform::python::find_system_python();

    let cli_folder_path: Option<String> =
        std::env::args()
            .nth(1)
            .and_then(|raw| match config::expand_and_canonicalize(&raw) {
                Ok(p) => Some(p.display().to_string()),
                Err(e) => {
                    eprintln!("Invalid folder argument '{}': {}", raw, e);
                    None
                }
            });

    let app_state = AppState {
        paths,
        read_only,
        gpu_info,
        running_scan_jobs: Arc::new(Mutex::new(HashSet::new())),
        setup_download: Arc::new(Mutex::new(llm::DownloadState::idle())),
        venv_provision: Arc::new(Mutex::new(platform::python::VenvProvisionState::idle())),
        cached_system_python,
        cancel_flags: Arc::new(Mutex::new(HashMap::new())),
        cli_folder_path,
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app_state)
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let _ = llm::cleanup_loaded_models();
            }
        })
        // CLI folder handling is delegated to the frontend via get_cli_folder_path.
        .invoke_handler(tauri::generate_handler![
            app_health,
            get_app_paths,
            get_cli_folder_path,
            ensure_database,
            parse_query_nl,
            get_setup_status,
            start_setup_download,
            start_venv_provision,
            search_images,
            start_scan,
            get_scan_job,
            list_active_scans,
            get_runtime_status,
            cancel_scan,
            remove_root,
            list_roots,
            list_subdirectories,
            load_user_config,
            save_user_config,
            copy_files_to_clipboard,
            delete_files,
            rename_file,
            get_file_metadata,
            get_file_properties,
            update_file_metadata,
            find_duplicates,
            create_album,
            delete_album,
            list_albums,
            add_files_to_album,
            remove_files_from_album,
            create_smart_folder,
            delete_smart_folder,
            list_smart_folders,
            reorder_roots,
            reorder_albums,
            reorder_smart_folders,
            add_pdf_password,
            delete_pdf_password,
            list_pdf_passwords,
            list_protected_pdfs,
            retry_protected_pdfs,
            reclassify_pdf,
            get_video_stream_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
