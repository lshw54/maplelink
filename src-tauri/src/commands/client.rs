//! Client manager: compare a local install against beanfun's manifest and
//! fetch back what does not match.
//!
//! The work itself lives in [`crate::services::client_manager`]. These wrappers
//! own the window, the one-job-at-a-time rule, and the progress events.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};

use crate::models::error::{ErrorCategory, ErrorDto};
use crate::services::client_manager::{
    self, Cancel, ClientManifest, Control, DownloadProgress, DownloadReport, LocalVersion,
    ScanMode, ScanReport,
};

/// The window the client manager runs in. `main.tsx` reads this label to decide
/// which UI to mount.
pub const CLIENT_WINDOW_LABEL: &str = "client_manager";

const SCAN_PROGRESS_EVENT: &str = "client-scan-progress";
const DOWNLOAD_PROGRESS_EVENT: &str = "client-download-progress";
/// How often a running download reports itself. Often enough to look live,
/// rarely enough that 1263 files do not flood the webview.
const TICK: std::time::Duration = std::time::Duration::from_millis(400);

/// One scan or download at a time, plus the manifest they share.
#[derive(Default)]
pub struct ClientJobs {
    manifest: tokio::sync::RwLock<Option<Arc<ClientManifest>>>,
    cancel: std::sync::Mutex<Option<Cancel>>,
    busy: AtomicBool,
}

impl ClientJobs {
    /// Claim the single job slot, handing back a guard that frees it.
    fn start(&self) -> Result<JobGuard<'_>, ErrorDto> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(err(
                "CLIENT_BUSY",
                "another scan or download is already running",
                ErrorCategory::Process,
            ));
        }
        let cancel: Cancel = Arc::new(Control::default());
        *self.cancel.lock().expect("client job lock") = Some(cancel.clone());
        Ok(JobGuard { jobs: self, cancel })
    }
}

struct JobGuard<'a> {
    jobs: &'a ClientJobs,
    cancel: Cancel,
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        *self.jobs.cancel.lock().expect("client job lock") = None;
        self.jobs.busy.store(false, Ordering::SeqCst);
    }
}

fn err(code: &str, message: impl Into<String>, category: ErrorCategory) -> ErrorDto {
    ErrorDto {
        code: code.to_string(),
        message: message.into(),
        category,
        details: None,
    }
}

fn net(code: &str) -> impl Fn(String) -> ErrorDto + '_ {
    move |e| err(code, e, ErrorCategory::Network)
}

/// Open (or focus) the client manager window.
#[tauri::command]
pub async fn open_client_manager_window(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    if let Some(existing) = app.get_webview_window(CLIENT_WINDOW_LABEL) {
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }
    // WebView2 refuses a second environment over the same user data folder
    // unless its options match the first one exactly (HRESULT 0x8007139F), so
    // this window has to ask for whatever the main window declared.
    let browser_args = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .and_then(|w| w.additional_browser_args.clone())
        .unwrap_or_default();

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        CLIENT_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .additional_browser_args(&browser_args)
    .title("MapleLink")
    // A file list plus progress needs far more room than the main window has.
    .inner_size(980.0, 680.0)
    .min_inner_size(720.0, 520.0)
    .resizable(true)
    // Borderless like the main window; the UI draws its own title bar, and the
    // global window-event handler rounds the corners through DWM.
    .decorations(false)
    .transparent(false)
    // The window shadow needs WS_THICKFRAME, and that frame's top edge is the
    // 1px accent hairline on Windows 11 that DWMWA_BORDER_COLOR cannot remove.
    // The main window drops the shadow for the same reason (tauri.conf.json5).
    .shadow(false)
    .center()
    .build()
    .map_err(|e| {
        err(
            "CLIENT_WINDOW_FAILED",
            format!("could not open the client manager window: {e}"),
            ErrorCategory::Process,
        )
    })?;

    // Strip the Windows 11 accent hairline and round the corners now, rather
    // than waiting for the first focus event — otherwise the line is visible
    // for as long as it takes the window to be focused.
    #[cfg(target_os = "windows")]
    crate::apply_borderless_dwm(&window.as_ref().window());

    Ok(())
}

/// Fetch the official manifest and remember it for the scan and download that
/// follow, so all three always talk about the same published version.
#[tauri::command]
pub async fn client_load_manifest(
    jobs: tauri::State<'_, ClientJobs>,
) -> Result<ClientManifest, ErrorDto> {
    let manifest = client_manager::fetch_manifest()
        .await
        .map_err(net("CLIENT_MANIFEST_FAILED"))?;
    let shared = Arc::new(manifest.clone());
    *jobs.manifest.write().await = Some(shared);
    Ok(manifest)
}

async fn manifest_of(jobs: &tauri::State<'_, ClientJobs>) -> Result<Arc<ClientManifest>, ErrorDto> {
    jobs.manifest.read().await.clone().ok_or_else(|| {
        err(
            "CLIENT_NO_MANIFEST",
            "load the manifest first",
            ErrorCategory::Process,
        )
    })
}

/// Compare `dir` against the manifest, reporting progress as it goes.
#[tauri::command]
pub async fn client_scan(
    dir: String,
    mode: ScanMode,
    app: tauri::AppHandle,
    jobs: tauri::State<'_, ClientJobs>,
) -> Result<ScanReport, ErrorDto> {
    let manifest = manifest_of(&jobs).await?;
    let guard = jobs.start()?;
    let handle = app.clone();
    let report = client_manager::scan(
        PathBuf::from(&dir),
        manifest,
        mode,
        guard.cancel.clone(),
        move |p| {
            if let Err(e) = handle.emit(SCAN_PROGRESS_EVENT, p) {
                tracing::warn!("client scan: progress emit failed: {e}");
            }
        },
    )
    .await
    .map_err(|e| err("CLIENT_SCAN_FAILED", e, ErrorCategory::FileSystem))?;
    tracing::info!(
        "client scan of {dir}: {} ok, {} to fetch, {} extra, cancelled={}",
        report.ok_files,
        report.issue_count,
        report.extra_files.len(),
        report.cancelled
    );
    Ok(report)
}

/// Fetch the named files into `dir`. Only paths the manifest lists are allowed,
/// and each one is verified before it replaces anything.
#[tauri::command]
pub async fn client_download(
    dir: String,
    paths: Vec<String>,
    direct: bool,
    app: tauri::AppHandle,
    jobs: tauri::State<'_, ClientJobs>,
) -> Result<DownloadReport, ErrorDto> {
    let manifest = manifest_of(&jobs).await?;
    let guard = jobs.start()?;
    let progress = Arc::new(DownloadProgress::default());

    // Report while the transfer runs; a single 186 MB file would otherwise be
    // a long silence.
    let ticker = {
        let (progress, app) = (progress.clone(), app.clone());
        let stop = guard.cancel.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(TICK);
            loop {
                timer.tick().await;
                let snapshot = progress.snapshot();
                let _ = app.emit(DOWNLOAD_PROGRESS_EVENT, &snapshot);
                if stop.is_cancelled() {
                    return;
                }
            }
        })
    };

    let report = client_manager::download(
        PathBuf::from(&dir),
        manifest,
        paths,
        direct,
        guard.cancel.clone(),
        progress.clone(),
    )
    .await
    .map_err(|e| err("CLIENT_DOWNLOAD_FAILED", e, ErrorCategory::Network));

    ticker.abort();
    // One last event so the bar always finishes where the report says it did.
    let _ = app.emit(DOWNLOAD_PROGRESS_EVENT, progress.snapshot());

    let report = report?;
    tracing::info!(
        "client download into {dir} (direct={direct}): {} of {} written, {} failed, cancelled={}",
        report.written,
        report.requested,
        report.failures.len(),
        report.cancelled
    );
    Ok(report)
}

/// Ask the running scan or download to stop. Safe to call when nothing runs.
#[tauri::command]
pub fn client_cancel(jobs: tauri::State<'_, ClientJobs>) {
    if let Some(cancel) = jobs.cancel.lock().expect("client job lock").as_ref() {
        cancel.cancel();
    }
}

/// Hold the running job, or let it continue. A paused job keeps its place, so
/// resuming costs nothing; cancelling a paused job still works.
#[tauri::command]
pub fn client_set_paused(paused: bool, jobs: tauri::State<'_, ClientJobs>) {
    if let Some(cancel) = jobs.cancel.lock().expect("client job lock").as_ref() {
        cancel.set_paused(paused);
    }
}

/// Free bytes on the volume holding `dir`, so the UI can warn before a 67 GB
/// download starts. `None` when the platform or the path cannot answer.
#[tauri::command]
pub fn client_free_space(dir: String) -> Option<u64> {
    client_manager::free_space(std::path::Path::new(&dir))
}

/// Pick the game folder. Starts at `start_in` when the caller has one, so the
/// player lands next to their existing install rather than at the drive root.
#[tauri::command]
pub async fn client_pick_folder(
    start_in: Option<String>,
    app: tauri::AppHandle,
) -> Result<Option<String>, ErrorDto> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    let mut dialog = app.dialog().file().set_title("Select the game folder");
    if let Some(dir) = start_in.filter(|d| !d.is_empty()) {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_folder(move |path| {
        let _ = tx.send(path.map(|p| p.to_string()));
    });
    rx.await.map_err(|_| {
        err(
            "CLIENT_DIALOG_FAILED",
            "folder dialog closed unexpectedly",
            ErrorCategory::Process,
        )
    })
}

/// The game folder the app already knows about, if the player has set one.
/// `game_path` points at `MapleStory.exe`; the manager works on its folder.
#[tauri::command]
pub async fn client_default_folder(
    state: tauri::State<'_, crate::models::app_state::AppState>,
) -> Result<Option<String>, ErrorDto> {
    let game_path = state.config.read().await.game_path.clone();
    if game_path.is_empty() {
        return Ok(None);
    }
    Ok(std::path::Path::new(&game_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string()))
}

/// What version the install in `dir` reports for itself, compared against the
/// published one. `None` when the folder holds no readable `Base.wz`.
#[tauri::command]
pub async fn client_local_version(
    dir: String,
    jobs: tauri::State<'_, ClientJobs>,
) -> Result<Option<LocalVersion>, ErrorDto> {
    let manifest = manifest_of(&jobs).await?;
    Ok(client_manager::local_version(
        std::path::Path::new(&dir),
        &manifest.version,
    ))
}
