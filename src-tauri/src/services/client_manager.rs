//! Compare a local MapleStory TW install against beanfun's official manifest,
//! and fetch back whatever does not match.
//!
//! This is the one place in MapleLink that writes into the player's game
//! folder, so the rules it follows are deliberate:
//!
//! - A file that already matches the manifest is never opened for writing.
//! - Every download lands in a `.mlpart` file next to its target and is only
//!   renamed over the real file once its SHA-256 equals the published one. A
//!   failed download leaves the existing file untouched.
//! - Nothing is ever deleted. Files the manifest does not mention are reported
//!   and left alone.
//! - Manifest paths arrive over the network, so every one is re-validated
//!   against the target folder before it can name a file on disk.
//!
//! A full install is the same code path as a repair: scanning an empty folder
//! reports every file as missing. That also gives resume for free — an
//! interrupted run is just a folder that scans as partly complete.

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// How many files to fetch at once. beanfun's CDN is fine with this and it
/// keeps a slow file from stalling the run, without looking like an attack.
const DOWNLOAD_CONCURRENCY: usize = 6;
/// Read buffer for hashing. Large enough that 67 GB does not die of syscalls.
const HASH_BUFFER: usize = 1024 * 1024;

/// One file as beanfun publishes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ManifestFile {
    /// Forward-slash relative path under the game folder.
    pub path: String,
    #[serde(rename = "sizeInBytes", deserialize_with = "flexible_u64")]
    pub size: u64,
    /// Lowercase hex SHA-256. Empty when beanfun published none.
    #[serde(default)]
    pub sha256: String,
}

/// The manifest, reduced to what the client manager needs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientManifest {
    pub product_name: String,
    pub version: String,
    /// "YYYY/MM/DD" as beanfun published it; empty when absent.
    pub publish_date: String,
    pub total_bytes: u64,
    pub file_count: usize,
    /// Where a file lives: `{base_url}{folder_name}/{path}`.
    #[serde(skip)]
    pub base_url: String,
    #[serde(skip)]
    pub folder_name: String,
    pub exe_name: String,
    /// Size of `ExePatch.dat` for this version, when the patch CDN answers.
    /// The manifest ships the base executable; the game's own updater replaces
    /// it with this build, so an install carrying it is current, not damaged.
    pub exe_patch_size: Option<u64>,
    /// When that build was published. beanfun replaces `ExePatch.dat` in place
    /// for a minor update without changing the version number, so this date
    /// marks which minor build the CDN is serving.
    pub exe_patch_date: Option<String>,
    /// The full version including the minor part, e.g. `"V282.2"`. `version`
    /// only ever carries the major (`"V282"`); the minor shows up in the names
    /// on beanfun's own download page and nowhere else public.
    pub full_version: Option<String>,
    /// RFC 3339 time the cached copy was fetched, when this manifest came from
    /// the cache instead of the network. `None` means it is fresh.
    pub cached_at: Option<String>,
    #[serde(skip)]
    pub files: Vec<ManifestFile>,
}

#[derive(Deserialize)]
struct RawInfo {
    #[serde(rename = "productName", default)]
    product_name: String,
    version: String,
    #[serde(rename = "publishDate", default)]
    publish_date: String,
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "executionPath", default)]
    execution_path: String,
    #[serde(default)]
    files: Vec<ManifestFile>,
}

/// `sizeInBytes` comes back as a number or as a string, depending on the entry.
fn flexible_u64<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    use serde::de::Error;
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| D::Error::custom("size is not a whole number")),
        serde_json::Value::String(s) => s.trim().parse().map_err(D::Error::custom),
        other => Err(D::Error::custom(format!("size is {other}"))),
    }
}

/// The game's own updater serves the runnable executable here, outside the
/// manifest. Plain HTTP: the host offers no TLS at all.
const EXE_PATCH_URL: &str =
    "http://tw.cdnpatch.maplestory.beanfun.com/maplestory/patch/patchdir/{:05}/ExePatch.dat";

/// `"V282"` -> `282`, so the patch CDN's zero-padded folder can be built.
fn version_number(version: &str) -> Option<u32> {
    version.trim().trim_start_matches(['V', 'v']).parse().ok()
}

pub fn exe_patch_url(version: &str) -> Option<String> {
    let n = version_number(version)?;
    Some(EXE_PATCH_URL.replace("{:05}", &format!("{n:05}")))
}

/// The cached manifest body, so a scan still works with no network.
const CACHE_FILE: &str = "client_manifest.json";

#[derive(Serialize, Deserialize)]
struct CachedManifest {
    /// RFC 3339, so the UI can say how old the copy is.
    fetched_at: String,
    /// Where the body came from. Resolving it needs the catalog, which is the
    /// first thing to go when beanfun is unreachable, so it is kept here too.
    #[serde(default)]
    url: String,
    /// The raw `productInfo.json` body, parsed the same way a fresh one is.
    body: String,
}

fn cache_path(dir: &Path) -> PathBuf {
    dir.join(CACHE_FILE)
}

fn read_cache(dir: &Path) -> Option<CachedManifest> {
    let text = std::fs::read_to_string(cache_path(dir)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_cache(dir: &Path, url: &str, body: &str) {
    let entry = CachedManifest {
        fetched_at: chrono::Utc::now().to_rfc3339(),
        url: url.to_string(),
        body: body.to_string(),
    };
    let Ok(json) = serde_json::to_string(&entry) else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    // Same temp-then-rename as prefs: a half-written cache must never be the
    // thing a later offline scan reads.
    let tmp = cache_path(dir).with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, cache_path(dir));
    }
}

/// Fetch and parse the official manifest, then ask the patch CDN how big the
/// runnable executable is for this version.
///
/// `cache_dir` is where the last good copy is kept. When beanfun cannot be
/// reached, that copy is used instead: a scan can still say which files are
/// wrong, even though repairing them needs the server.
pub async fn fetch_manifest(cache_dir: &Path) -> Result<ClientManifest, String> {
    let (body, cached_at, url) = match crate::services::game_download::fetch_product_info_body().await
    {
        Ok((url, body)) => {
            write_cache(cache_dir, &url, &body);
            (body, None, url)
        }
        Err(e) => {
            let cached = read_cache(cache_dir).ok_or_else(|| {
                format!("{e} (and no cached manifest to fall back on)")
            })?;
            tracing::info!(
                "client manager: beanfun unreachable ({e}); using the copy cached at {}",
                cached.fetched_at
            );
            (cached.body, Some(cached.fetched_at), cached.url)
        }
    };
    let mut manifest = parse_manifest(&body)?;
    manifest.cached_at = cached_at;
    let _ = url;
    let (size, date) = probe_exe_patch(&manifest.version).await;
    manifest.exe_patch_size = size;
    manifest.exe_patch_date = date;
    manifest.full_version = match crate::services::game_download::fetch_download_list().await {
        Ok(items) => {
            let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
            minor_version(&names, &manifest.version)
        }
        Err(e) => {
            tracing::info!("client manager: download list unavailable for minor version: {e}");
            None
        }
    };
    Ok(manifest)
}

/// Size and publish date of this version's `ExePatch.dat`. Best effort: a scan
/// still works without them, it just cannot tell a self-patched executable from
/// a damaged one.
async fn probe_exe_patch(version: &str) -> (Option<u64>, Option<String>) {
    match probe_exe_patch_inner(version).await {
        Some((size, date)) => (Some(size), date),
        None => (None, None),
    }
}

async fn probe_exe_patch_inner(version: &str) -> Option<(u64, Option<String>)> {
    let url = exe_patch_url(version)?;
    let client = reqwest::Client::builder()
        .user_agent(crate::services::http_util::USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let resp = client.head(&url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::info!(
            "client manager: no ExePatch for {version} (HTTP {})",
            resp.status()
        );
        return None;
    }
    // beanfun replaces this file in place for a minor update without moving the
    // version number, so its date is the only public marker of the minor build.
    let date = resp
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    // `content_length()` reports the body length, which is 0 for a HEAD
    // response; the header is the only place the real size shows up.
    let size = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    tracing::info!("client manager: ExePatch for {version} is {size} bytes, published {date:?}");
    Some((size, date))
}

fn parse_manifest(body: &str) -> Result<ClientManifest, String> {
    let raw: RawInfo =
        serde_json::from_str(body).map_err(|e| format!("failed to parse product info: {e}"))?;
    if raw.version.trim().is_empty() {
        return Err("product info has no version".to_string());
    }
    let mut base = raw.base_url.trim().to_string();
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err("product info has no usable baseUrl".to_string());
    }
    if !base.ends_with('/') {
        base.push('/');
    }
    let mut segments = raw.execution_path.split('/').filter(|s| !s.is_empty());
    let folder_name = segments.next().unwrap_or_default().to_string();
    let exe_name = segments.next_back().unwrap_or_default().to_string();
    if folder_name.is_empty() {
        return Err("product info names no game folder".to_string());
    }
    if raw.files.is_empty() {
        return Err("product info lists no files".to_string());
    }
    // Every path must be usable before anything else runs.
    for f in &raw.files {
        check_relative_path(&f.path)?;
    }
    Ok(ClientManifest {
        product_name: raw.product_name,
        version: raw.version,
        publish_date: raw.publish_date,
        total_bytes: raw.files.iter().map(|f| f.size).sum(),
        file_count: raw.files.len(),
        base_url: base,
        folder_name,
        exe_name,
        exe_patch_size: None,
        exe_patch_date: None,
        full_version: None,
        cached_at: None,
        files: raw.files,
    })
}

/// Reject anything that could name a file outside the game folder. Manifest
/// paths come off the network; this is the gate that makes them safe to join.
fn check_relative_path(rel: &str) -> Result<(), String> {
    if rel.is_empty() {
        return Err("manifest has an empty path".to_string());
    }
    if rel.starts_with('/') || rel.len() >= 2 && rel.as_bytes()[1] == b':' {
        return Err(format!("manifest path is absolute: {rel:?}"));
    }
    for seg in rel.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(format!("manifest path has a bad segment: {rel:?}"));
        }
        if seg.contains('\\') || seg.contains(':') {
            return Err(format!("manifest path has a bad segment: {rel:?}"));
        }
        // Windows strips a trailing dot or space, which would let two manifest
        // entries collide on one real file.
        if seg.ends_with('.') || seg.ends_with(' ') {
            return Err(format!("manifest path has a bad segment: {rel:?}"));
        }
    }
    Ok(())
}

/// Join a validated manifest path onto the game folder.
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    check_relative_path(rel)?;
    let mut out = root.to_path_buf();
    for seg in rel.split('/') {
        out.push(seg);
    }
    Ok(out)
}

/// How hard to look. `Quick` only compares sizes, which catches missing and
/// truncated files in seconds; `Full` hashes every byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    Quick,
    Full,
}

/// Why a file needs fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueKind {
    Missing,
    SizeMismatch,
    HashMismatch,
    Unreadable,
}

/// Every file the scan looked at, whatever the verdict. The UI lists the
/// untouched ones too, so a player can see what was left alone rather than
/// only what went wrong.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckedFile {
    pub path: String,
    /// `None` when the file matches the manifest.
    pub kind: Option<IssueKind>,
    pub expected_size: u64,
    pub local_size: Option<u64>,
}

impl CheckedFile {
    pub fn is_issue(&self) -> bool {
        self.kind.is_some()
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub total_files: usize,
    pub ok_files: usize,
    /// Every file examined, in manifest order.
    pub files: Vec<CheckedFile>,
    pub issue_count: usize,
    pub bytes_to_fetch: u64,
    /// Present on disk, absent from the manifest. Reported only; never removed.
    pub extra_files: Vec<String>,
    pub cancelled: bool,
}

/// Progress during a scan or a download.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current: String,
}

/// Shared run control: stop a job, or hold it without losing its place.
#[derive(Debug, Default)]
pub struct Control {
    cancelled: AtomicBool,
    paused: AtomicBool,
}

/// How often a paused job looks up to see whether it may continue.
const PAUSE_POLL: std::time::Duration = std::time::Duration::from_millis(120);

impl Control {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        // A paused job must wake up to notice it was cancelled.
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Block while paused. `false` means the job should stop.
    fn hold_blocking(&self) -> bool {
        while self.is_paused() && !self.is_cancelled() {
            std::thread::sleep(PAUSE_POLL);
        }
        !self.is_cancelled()
    }

    /// The same, for a job running on the async runtime.
    async fn hold_async(&self) -> bool {
        while self.is_paused() && !self.is_cancelled() {
            tokio::time::sleep(PAUSE_POLL).await;
        }
        !self.is_cancelled()
    }
}

pub type Cancel = Arc<Control>;

/// Compare `dir` against the manifest.
///
/// Runs on a blocking thread: hashing a full install reads ~67 GB.
pub async fn scan(
    dir: PathBuf,
    manifest: Arc<ClientManifest>,
    mode: ScanMode,
    cancel: Cancel,
    on_progress: impl Fn(Progress) + Send + Sync + 'static,
) -> Result<ScanReport, String> {
    tokio::task::spawn_blocking(move || scan_blocking(&dir, &manifest, mode, &cancel, &on_progress))
        .await
        .map_err(|e| format!("scan task failed: {e}"))?
}

/// Two passes, because they cost wildly different amounts.
///
/// The first only asks the filesystem for sizes: 1263 of those take a moment
/// and already catch everything missing or truncated. The second hashes what
/// survived, which is the expensive part — so it is the only one that runs in
/// parallel, and the only one whose progress is measured in bytes.
fn scan_blocking(
    dir: &Path,
    manifest: &ClientManifest,
    mode: ScanMode,
    cancel: &Cancel,
    on_progress: &(impl Fn(Progress) + Send + Sync),
) -> Result<ScanReport, String> {
    let total = manifest.files.len();
    let mut report = ScanReport {
        total_files: total,
        ..Default::default()
    };

    // ---- pass 1: metadata -------------------------------------------------
    let mut pending: Vec<usize> = Vec::new();
    for (i, f) in manifest.files.iter().enumerate() {
        if !cancel.hold_blocking() {
            report.cancelled = true;
            return Ok(report);
        }
        let target = safe_join(dir, &f.path)?;
        // The executable legitimately differs from the manifest once the game
        // has patched itself, so it is measured against that build as well.
        let self_patched = (f.path == manifest.exe_name)
            .then_some(manifest.exe_patch_size)
            .flatten();
        let (kind, needs_hash) = inspect_metadata(&target, f, mode, self_patched);
        if needs_hash {
            pending.push(i);
        }
        report.files.push(CheckedFile {
            path: f.path.clone(),
            kind,
            expected_size: f.size,
            local_size: std::fs::metadata(&target).ok().map(|m| m.len()),
        });
        if i % 50 == 0 || i + 1 == total {
            on_progress(Progress {
                done: i + 1,
                total,
                bytes_done: 0,
                bytes_total: 0,
                current: f.path.clone(),
            });
        }
    }

    // ---- pass 2: hashing --------------------------------------------------
    if !pending.is_empty() {
        let bytes_total: u64 = pending.iter().map(|&i| manifest.files[i].size).sum();
        let verdicts = hash_in_parallel(dir, manifest, &pending, cancel, bytes_total, on_progress)?;
        for (&i, kind) in pending.iter().zip(verdicts) {
            report.files[i].kind = kind;
        }
    }

    if cancel.is_cancelled() {
        report.cancelled = true;
    }
    for f in &report.files {
        if f.is_issue() {
            report.issue_count += 1;
            report.bytes_to_fetch += f.expected_size;
        } else {
            report.ok_files += 1;
        }
    }
    if !report.cancelled {
        report.extra_files = find_extra_files(dir, manifest, cancel);
    }
    Ok(report)
}

/// Hash the pending files across a few threads.
///
/// Reading several files at once is a large win on flash and a large loss on a
/// spinning disk, where the heads end up seeking between them — so a spinning
/// disk gets one worker.
fn hash_in_parallel(
    dir: &Path,
    manifest: &ClientManifest,
    pending: &[usize],
    cancel: &Cancel,
    bytes_total: u64,
    on_progress: &(impl Fn(Progress) + Send + Sync),
) -> Result<Vec<Option<IssueKind>>, String> {
    let workers = hash_workers(dir).min(pending.len());
    let next = AtomicUsize::new(0);
    let bytes_done = AtomicU64::new(0);
    let done = AtomicUsize::new(0);
    let out: Vec<std::sync::Mutex<Option<IssueKind>>> = (0..pending.len())
        .map(|_| std::sync::Mutex::new(None))
        .collect();

    tracing::info!(
        "client scan: hashing {} files ({} bytes) with {workers} worker(s)",
        pending.len(),
        bytes_total
    );

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                if !cancel.hold_blocking() {
                    return;
                }
                let slot = next.fetch_add(1, Ordering::Relaxed);
                let Some(&index) = pending.get(slot) else {
                    return;
                };
                let f = &manifest.files[index];
                let Ok(target) = safe_join(dir, &f.path) else {
                    *out[slot].lock().expect("hash slot") = Some(IssueKind::Unreadable);
                    continue;
                };
                let verdict = match hash_file(&target) {
                    Ok(got) if got.eq_ignore_ascii_case(&f.sha256) => None,
                    Ok(_) => Some(IssueKind::HashMismatch),
                    Err(_) => Some(IssueKind::Unreadable),
                };
                *out[slot].lock().expect("hash slot") = verdict;

                let seen = bytes_done.fetch_add(f.size, Ordering::Relaxed) + f.size;
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                on_progress(Progress {
                    done: n,
                    total: pending.len(),
                    bytes_done: seen,
                    bytes_total,
                    current: f.path.clone(),
                });
            });
        }
    });

    Ok(out
        .into_iter()
        .map(|m| m.into_inner().expect("hash slot"))
        .collect())
}

/// One worker on a spinning disk, a handful on anything else.
fn hash_workers(dir: &Path) -> usize {
    if spinning_disk(dir).unwrap_or(false) {
        return 1;
    }
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    // Past about four readers the drive, not the CPU, is the limit.
    cores.clamp(1, 4)
}

/// What the filesystem alone can say. The second element asks for a hash: the
/// file is the right size, so only its contents are still in question.
fn inspect_metadata(
    target: &Path,
    f: &ManifestFile,
    mode: ScanMode,
    self_patched_size: Option<u64>,
) -> (Option<IssueKind>, bool) {
    let meta = match std::fs::metadata(target) {
        Ok(m) => m,
        Err(_) => return (Some(IssueKind::Missing), false),
    };
    if !meta.is_file() {
        return (Some(IssueKind::Missing), false);
    }
    // Matching the build the game patches itself to is correct, and there is no
    // published hash for it, so the check ends here.
    if Some(meta.len()) == self_patched_size {
        return (None, false);
    }
    if meta.len() != f.size {
        return (Some(IssueKind::SizeMismatch), false);
    }
    (None, mode == ScanMode::Full && !f.sha256.is_empty())
}

/// Whether the volume holding `dir` is a spinning disk. `None` when the system
/// will not say, which is treated as "not spinning" by the caller.
#[cfg(target_os = "windows")]
fn spinning_disk(dir: &Path) -> Option<bool> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::{
        StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
        IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    // `\\.\C:` — the volume the path sits on.
    let root = dir.components().next()?;
    let mut path: Vec<u16> = std::ffi::OsStr::new(r"\\.\")
        .encode_wide()
        .chain(root.as_os_str().encode_wide().take(2))
        .collect();
    path.push(0);

    // SAFETY: `path` is a NUL-terminated UTF-16 device path; the query and
    // descriptor are plain PODs sized by `size_of`, and the handle is closed on
    // every path out.
    unsafe {
        let handle = CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceSeekPenaltyProperty,
            QueryType: 0,
            AdditionalParameters: [0],
        };
        let mut desc: DEVICE_SEEK_PENALTY_DESCRIPTOR = std::mem::zeroed();
        let mut returned = 0u32;
        let ok = DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const _,
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            &mut desc as *mut _ as *mut _,
            std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        );
        CloseHandle(handle);
        (ok != 0).then_some(desc.IncursSeekPenalty)
    }
}

#[cfg(not(target_os = "windows"))]
fn spinning_disk(_dir: &Path) -> Option<bool> {
    None
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; HASH_BUFFER];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Files on disk the manifest does not list. Reported so the player can decide;
/// the client manager never removes anything itself.
fn find_extra_files(dir: &Path, manifest: &ClientManifest, cancel: &Cancel) -> Vec<String> {
    use std::collections::HashSet;
    let known: HashSet<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
    let mut extra = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if cancel.is_cancelled() || extra.len() >= 500 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(rel) = p.strip_prefix(dir) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            // Our own in-flight downloads are not the player's stray files.
            if rel.ends_with(PART_SUFFIX) {
                continue;
            }
            if !known.contains(rel.as_str()) {
                extra.push(rel);
            }
        }
    }
    extra.sort();
    extra
}

const PART_SUFFIX: &str = ".mlpart";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadReport {
    pub requested: usize,
    pub written: usize,
    pub failures: Vec<DownloadFailure>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFailure {
    pub path: String,
    pub error: String,
}

/// Live counters for a download, so a caller can report smooth progress while
/// a single large file is still in flight.
#[derive(Debug, Default)]
pub struct DownloadProgress {
    pub files_done: AtomicU64,
    pub files_total: AtomicU64,
    pub bytes_done: AtomicU64,
    pub bytes_total: AtomicU64,
}

impl DownloadProgress {
    pub fn snapshot(&self) -> Progress {
        Progress {
            done: self.files_done.load(Ordering::Relaxed) as usize,
            total: self.files_total.load(Ordering::Relaxed) as usize,
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            current: String::new(),
        }
    }
}

/// Fetch the named manifest paths into `dir`.
///
/// Each file is written beside its target and renamed over it only after its
/// SHA-256 matches what beanfun published, so a failure anywhere leaves the
/// existing file exactly as it was.
/// `direct` bypasses the system proxy. Players running a game accelerator
/// often reach beanfun's CDN faster without it, and the CDN is the same host
/// either way — so this is a choice, not a default.
pub async fn download(
    dir: PathBuf,
    manifest: Arc<ClientManifest>,
    paths: Vec<String>,
    direct: bool,
    cancel: Cancel,
    progress: Arc<DownloadProgress>,
) -> Result<DownloadReport, String> {
    use futures_util::stream::StreamExt;

    let wanted: std::collections::HashSet<String> = paths.into_iter().collect();
    let files: Vec<ManifestFile> = manifest
        .files
        .iter()
        .filter(|f| wanted.contains(&f.path))
        .cloned()
        .collect();
    if files.len() != wanted.len() {
        return Err("asked for a file the manifest does not list".to_string());
    }

    let total = files.len();
    progress.files_total.store(total as u64, Ordering::Relaxed);
    progress
        .bytes_total
        .store(files.iter().map(|f| f.size).sum(), Ordering::Relaxed);
    let mut builder = reqwest::Client::builder()
        .user_agent(crate::services::http_util::USER_AGENT)
        // No overall timeout: a 186 MB file on a slow line is not a failure.
        .connect_timeout(std::time::Duration::from_secs(20));
    if direct {
        builder = builder.no_proxy();
    }
    let client = builder
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let failures = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    // Counted on success only. Deriving it from `total - failures` would call
    // every file a cancelled run never reached "written".
    let written = Arc::new(AtomicUsize::new(0));

    futures_util::stream::iter(files)
        .for_each_concurrent(DOWNLOAD_CONCURRENCY, |f| {
            let (client, dir, manifest, cancel) = (
                client.clone(),
                dir.clone(),
                manifest.clone(),
                cancel.clone(),
            );
            let (failures, progress, written) =
                (failures.clone(), progress.clone(), written.clone());
            async move {
                if !cancel.hold_async().await {
                    return;
                }
                // Bytes already counted for this file, so a retry or a failure
                // can be rolled back out of the running total.
                let counted = Arc::new(AtomicU64::new(0));
                let tally = counted.clone();
                let p = progress.clone();
                let result = fetch_one(&client, &dir, &manifest, &f, &cancel, move |n| {
                    tally.fetch_add(n, Ordering::Relaxed);
                    p.bytes_done.fetch_add(n, Ordering::Relaxed);
                })
                .await;
                if result.is_ok() {
                    written.fetch_add(1, Ordering::Relaxed);
                }
                if let Err(e) = result {
                    // The file did not land, so its bytes are not progress.
                    progress
                        .bytes_done
                        .fetch_sub(counted.load(Ordering::Relaxed), Ordering::Relaxed);
                    if !cancel.is_cancelled() {
                        tracing::warn!("client manager: {} failed: {e}", f.path);
                        failures.lock().await.push(DownloadFailure {
                            path: f.path.clone(),
                            error: e,
                        });
                    }
                }
                progress.files_done.fetch_add(1, Ordering::Relaxed);
            }
        })
        .await;

    let failures = failures.lock().await.clone();
    Ok(DownloadReport {
        requested: total,
        written: written.load(Ordering::Relaxed),
        failures,
        cancelled: cancel.is_cancelled(),
    })
}

/// Download one file, verify it, then put it in place.
async fn fetch_one(
    client: &reqwest::Client,
    dir: &Path,
    manifest: &ClientManifest,
    f: &ManifestFile,
    cancel: &Cancel,
    mut on_bytes: impl FnMut(u64),
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let target = safe_join(dir, &f.path)?;
    let part = target.with_extension(format!(
        "{}{}",
        target
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default(),
        PART_SUFFIX
    ));
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }

    let url = format!("{}{}/{}", manifest.base_url, manifest.folder_name, f.path);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let mut file = tokio::fs::File::create(&part)
        .await
        .map_err(|e| format!("could not open {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut written = 0u64;
    let mut stream = resp.bytes_stream();
    let outcome = loop {
        // Pausing mid-file matters: one of these is 177 MB.
        if !cancel.hold_async().await {
            break Err("cancelled".to_string());
        }
        match stream.next().await {
            None => break Ok(()),
            Some(Err(e)) => break Err(format!("transfer failed: {e}")),
            Some(Ok(chunk)) => {
                written += chunk.len() as u64;
                if written > f.size {
                    break Err("server sent more than the manifest states".to_string());
                }
                hasher.update(&chunk);
                if let Err(e) = file.write_all(&chunk).await {
                    break Err(format!("write failed: {e}"));
                }
                on_bytes(chunk.len() as u64);
            }
        }
    };
    let flushed = file.flush().await.map_err(|e| format!("flush failed: {e}"));
    drop(file);

    let verdict = outcome.and(flushed).and_then(|()| {
        if written != f.size {
            return Err(format!("got {written} bytes, manifest says {}", f.size));
        }
        let got = hex(&hasher.finalize());
        if !f.sha256.is_empty() && !got.eq_ignore_ascii_case(&f.sha256) {
            return Err("SHA-256 does not match the manifest".to_string());
        }
        Ok(())
    });

    match verdict {
        Ok(()) => {
            // Only now does the player's file change.
            tokio::fs::rename(&part, &target)
                .await
                .map_err(|e| format!("could not replace {}: {e}", target.display()))
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(e)
        }
    }
}

/// Pull the full version out of beanfun's download-page item names.
///
/// The manifest only ever states the major (`"V282"`), but the download page
/// lists entries like `【官方載點】V282.2 手動更新`. Only minors belonging to
/// the published major count, and the highest one wins.
fn minor_version(names: &[&str], version: &str) -> Option<String> {
    let major = version_number(version)?;
    let pattern = regex::Regex::new(r"[Vv]?(\d+)\.(\d+)").ok()?;
    let best = names
        .iter()
        .flat_map(|name| pattern.captures_iter(name))
        .filter_map(|c| {
            let found: u32 = c.get(1)?.as_str().parse().ok()?;
            let minor: u32 = c.get(2)?.as_str().parse().ok()?;
            (found == major).then_some(minor)
        })
        .max()?;
    Some(format!("V{major}.{best}"))
}

/// What the installed client's own data says about its version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalVersion {
    /// The 8-bit marker stored in `Data/Base/Base.wz`.
    pub marker: u16,
    /// Whether that marker is the one the published version would produce.
    pub matches_official: bool,
    /// Versions that would produce this marker. It is only a checksum, so a
    /// few distant versions collide; the list is shown rather than guessed at.
    pub candidates: Vec<u32>,
}

/// MapleStory's version marker: a checksum over the decimal digits of the
/// version, stored in the WZ header. Several versions share one value, so it
/// confirms a version rather than naming one.
fn version_marker(version: u32) -> u16 {
    let mut hash: u32 = 0;
    for ch in version.to_string().bytes() {
        hash = hash.wrapping_mul(32).wrapping_add(ch as u32 + 1);
    }
    let [a, b, c, d] = hash.to_be_bytes();
    (0xFF ^ a ^ b ^ c ^ d) as u16
}

/// Read the marker out of `Data/Base/Base.wz`.
fn read_wz_marker(dir: &Path) -> Option<u16> {
    use std::io::Read;

    let path = dir.join("Data").join("Base").join("Base.wz");
    let mut head = [0u8; 4096];
    let read = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    if read < 16 || &head[0..4] != b"PKG1" {
        return None;
    }
    let data_start = u32::from_le_bytes([head[12], head[13], head[14], head[15]]) as usize;
    if data_start + 2 > read {
        return None;
    }
    Some(u16::from_le_bytes([head[data_start], head[data_start + 1]]))
}

/// Identify the installed client's version against the published one.
pub fn local_version(dir: &Path, official: &str) -> Option<LocalVersion> {
    let marker = read_wz_marker(dir)?;
    let official_number = version_number(official);
    Some(LocalVersion {
        marker,
        matches_official: official_number.is_some_and(|v| version_marker(v) == marker),
        // Far enough to cover any version this client will plausibly be on.
        candidates: (1..=2000)
            .filter(|v| version_marker(*v) == marker)
            .collect(),
    })
}

/// Free bytes on the volume holding `dir`, for the "do you have room" check.
#[cfg(target_os = "windows")]
pub fn free_space(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut wide: Vec<u16> = dir.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut free: u64 = 0;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call,
    // and `free` is a valid writable u64 for the out-parameter.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
}

#[cfg(not(target_os = "windows"))]
pub fn free_space(_dir: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp folder that removes itself; the repo has no tempdir crate.
    pub(super) struct TempDir(pub PathBuf);

    impl TempDir {
        pub(super) fn new(tag: &str) -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "maplelink_{tag}_{}_{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }

        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const INFO: &str = r#"{
        "productName":"新楓之谷","productId":"MS","sizeInBytes":3,
        "version":"V282","publishDate":"2026/09/04",
        "baseUrl":"https://cdn.example.com/maplestory/download/",
        "executionPath":"P2PdPoyK5obH/MapleStory.exe",
        "files":[
            {"path":"a.txt","sizeInBytes":5,"sha256":"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"},
            {"path":"sub/b.bin","sizeInBytes":"3","sha256":"a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"}
        ]
    }"#;

    fn manifest() -> ClientManifest {
        parse_manifest(INFO).unwrap()
    }

    #[test]
    fn parses_sizes_given_as_numbers_or_strings() {
        let m = manifest();
        assert_eq!(m.version, "V282");
        assert_eq!(m.folder_name, "P2PdPoyK5obH");
        assert_eq!(m.exe_name, "MapleStory.exe");
        assert_eq!(m.file_count, 2);
        assert_eq!(m.files[0].size, 5);
        assert_eq!(m.files[1].size, 3);
        assert_eq!(m.total_bytes, 8);
    }

    #[test]
    fn a_manifest_without_files_or_version_is_refused() {
        assert!(parse_manifest(&INFO.replace("\"V282\"", "\"\"")).is_err());
        let no_files = INFO.replace("\"files\":[", "\"files\":[],\"unused\":[");
        assert!(parse_manifest(&no_files).is_err());
    }

    #[test]
    fn a_manifest_that_tries_to_escape_the_game_folder_is_refused() {
        for bad in [
            "../evil.exe",
            "a/../../evil.exe",
            "/etc/passwd",
            "C:/Windows/system32/evil.dll",
            "sub/",
            "trailingdot.",
        ] {
            let body = INFO.replace("a.txt", bad);
            assert!(
                parse_manifest(&body).is_err(),
                "{bad:?} should have been refused"
            );
        }
    }

    /// Separators and drive letters are checked against the validator itself:
    /// routing them through the JSON fixture would turn `\b` into an escape
    /// rather than the backslash the test means to feed it.
    #[test]
    fn the_path_validator_refuses_separators_and_drive_letters() {
        for bad in [
            "sub\\b.bin",
            "..\\evil.exe",
            "C:\\Windows\\evil.dll",
            "a:b",
            "../evil.exe",
            "",
            "ends.",
            "ends ",
        ] {
            assert!(
                check_relative_path(bad).is_err(),
                "{bad:?} should have been refused"
            );
        }
        for good in ["a.txt", "sub/b.bin", "a/b/c/d.dat", "Data-1_x.wz"] {
            assert!(check_relative_path(good).is_ok(), "{good:?} should be fine");
        }
    }

    #[test]
    fn safe_join_builds_paths_under_the_root_only() {
        let root = Path::new("C:\\Games\\MapleStory");
        assert_eq!(
            safe_join(root, "sub/b.bin").unwrap(),
            root.join("sub").join("b.bin")
        );
        assert!(safe_join(root, "../x").is_err());
        assert!(safe_join(root, "").is_err());
    }

    /// A folder holding `a.txt` correct, `sub/b.bin` corrupted at the same
    /// size, plus a file the manifest never mentions.
    fn sample_dir() -> TempDir {
        let dir = TempDir::new("mlscan");
        std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub").join("b.bin"), b"XXX").unwrap();
        std::fs::write(dir.path().join("mine.ini"), b"keep me").unwrap();
        dir
    }

    #[tokio::test]
    async fn a_quick_scan_sees_sizes_and_a_full_scan_sees_content() {
        let dir = sample_dir();
        let m = Arc::new(manifest());
        let cancel: Cancel = Arc::new(Control::default());

        let quick = scan(
            dir.path().to_path_buf(),
            m.clone(),
            ScanMode::Quick,
            cancel.clone(),
            |_| {},
        )
        .await
        .unwrap();
        // Both files are the right size, so a quick scan is happy.
        assert_eq!(quick.ok_files, 2);
        assert_eq!(quick.issue_count, 0);
        assert_eq!(quick.files.len(), 2);

        let full = scan(dir.path().to_path_buf(), m, ScanMode::Full, cancel, |_| {})
            .await
            .unwrap();
        assert_eq!(full.ok_files, 1);
        assert_eq!(full.issue_count, 1);
        let bad: Vec<&CheckedFile> = full.files.iter().filter(|f| f.is_issue()).collect();
        assert_eq!(bad[0].path, "sub/b.bin");
        assert_eq!(bad[0].kind, Some(IssueKind::HashMismatch));
        // The untouched file is still reported, so the UI can show it.
        assert!(full
            .files
            .iter()
            .any(|f| f.path == "a.txt" && !f.is_issue()));
        assert_eq!(full.bytes_to_fetch, 3);
        assert_eq!(full.extra_files, vec!["mine.ini".to_string()]);
    }

    #[tokio::test]
    async fn an_empty_folder_reports_every_file_missing() {
        let dir = TempDir::new("mlempty");
        let m = Arc::new(manifest());
        let report = scan(
            dir.path().to_path_buf(),
            m,
            ScanMode::Quick,
            Arc::new(Control::default()),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(report.ok_files, 0);
        assert_eq!(report.issue_count, 2);
        assert!(report
            .files
            .iter()
            .all(|f| f.kind == Some(IssueKind::Missing) && f.local_size.is_none()));
        assert_eq!(report.bytes_to_fetch, 8);
    }

    #[tokio::test]
    async fn a_truncated_file_is_caught_without_hashing() {
        let dir = sample_dir();
        std::fs::write(dir.path().join("a.txt"), b"hi").unwrap();
        let report = scan(
            dir.path().to_path_buf(),
            Arc::new(manifest()),
            ScanMode::Quick,
            Arc::new(Control::default()),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(report.issue_count, 1);
        let bad = report.files.iter().find(|f| f.is_issue()).unwrap();
        assert_eq!(bad.kind, Some(IssueKind::SizeMismatch));
        assert_eq!(bad.local_size, Some(2));
    }

    #[tokio::test]
    async fn cancelling_stops_the_scan_and_says_so() {
        let dir = sample_dir();
        let cancel: Cancel = Arc::new(Control::default());
        cancel.cancel();
        let report = scan(
            dir.path().to_path_buf(),
            Arc::new(manifest()),
            ScanMode::Full,
            cancel,
            |_| {},
        )
        .await
        .unwrap();
        assert!(report.cancelled);
    }

    #[tokio::test]
    async fn progress_reaches_the_last_file() {
        let dir = sample_dir();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        scan(
            dir.path().to_path_buf(),
            Arc::new(manifest()),
            ScanMode::Quick,
            Arc::new(Control::default()),
            move |p| sink.lock().unwrap().push(p),
        )
        .await
        .unwrap();
        let seen = seen.lock().unwrap();
        let last = seen.last().unwrap();
        assert_eq!(last.done, 2);
        assert_eq!(last.total, 2);
        assert_eq!(last.bytes_done, last.bytes_total);
    }

    #[test]
    fn pausing_holds_a_run_and_cancelling_releases_it() {
        let control = Control::default();
        assert!(!control.is_paused());
        // Not paused: the check returns at once and says carry on.
        assert!(control.hold_blocking());

        control.set_paused(true);
        assert!(control.is_paused());

        // Cancelling a paused run must wake it, or it would hold forever.
        control.cancel();
        assert!(!control.is_paused());
        assert!(!control.hold_blocking());
        assert!(control.is_cancelled());
    }

    #[test]
    fn the_minor_version_comes_from_the_download_page_names() {
        let names = [
            "遊戲橘子遊戲管理器(推薦)",
            "【官方載點】V282.2 手動更新",
            "【官方載點】V281~V282",
            "【官方載點】V280~V282",
        ];
        assert_eq!(minor_version(&names, "V282").as_deref(), Some("V282.2"));
        // A minor belonging to another major is not this client's.
        assert_eq!(minor_version(&names, "V281"), None);
        // The highest minor wins.
        let later = ["V282.2 手動更新", "V282.10 手動更新"];
        assert_eq!(minor_version(&later, "V282").as_deref(), Some("V282.10"));
        // Nothing to find, and an unparsable version, both come back empty.
        assert_eq!(minor_version(&["完整程式"], "V282"), None);
        assert_eq!(minor_version(&names, "L.250508.1_2"), None);
    }

    #[test]
    fn the_version_marker_matches_the_one_a_real_client_carries() {
        // A live V282 install stores 127 in Data/Base/Base.wz.
        assert_eq!(version_marker(282), 127);
        // It is only a checksum, so distant versions collide — which is why
        // the UI reports candidates instead of naming one.
        let sharing: Vec<u32> = (1..=500).filter(|v| version_marker(*v) == 127).collect();
        assert!(sharing.contains(&282));
        assert!(sharing.len() > 1);
        assert_ne!(version_marker(281), version_marker(282));
    }

    #[test]
    fn the_local_version_is_read_from_base_wz() {
        let dir = TempDir::new("mlwz");
        assert!(local_version(dir.path(), "V282").is_none());

        // PKG1 header: data_start at offset 12, marker at data_start.
        let base = dir.path().join("Data").join("Base");
        std::fs::create_dir_all(&base).unwrap();
        let mut wz = vec![0u8; 64];
        wz[0..4].copy_from_slice(b"PKG1");
        wz[12..16].copy_from_slice(&60u32.to_le_bytes());
        wz[60..62].copy_from_slice(&127u16.to_le_bytes());
        std::fs::write(base.join("Base.wz"), &wz).unwrap();

        let got = local_version(dir.path(), "V282").unwrap();
        assert_eq!(got.marker, 127);
        assert!(got.matches_official);
        assert!(got.candidates.contains(&282));

        // The same install against a different published version does not match.
        assert!(!local_version(dir.path(), "V281").unwrap().matches_official);
    }

    #[test]
    fn a_file_that_is_not_a_wz_container_is_refused() {
        let dir = TempDir::new("mlwzbad");
        let base = dir.path().join("Data").join("Base");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("Base.wz"), b"not a wz file at all").unwrap();
        assert!(local_version(dir.path(), "V282").is_none());
    }

    #[test]
    fn the_patch_cdn_folder_comes_from_the_manifest_version() {
        assert_eq!(version_number("V282"), Some(282));
        assert_eq!(version_number(" v7 "), Some(7));
        assert_eq!(version_number("L.250508.1_2"), None);
        assert_eq!(
            exe_patch_url("V282").unwrap(),
            "http://tw.cdnpatch.maplestory.beanfun.com/maplestory/patch/patchdir/00282/ExePatch.dat"
        );
        assert!(exe_patch_url("nonsense").is_none());
    }

    /// The manifest ships the base executable; the game replaces it with the
    /// ExePatch build. An install carrying that build is current, not damaged.
    #[tokio::test]
    async fn an_executable_matching_the_self_patched_build_is_not_an_issue() {
        let dir = sample_dir();
        // `a.txt` stands in for the executable: right file, unexpected size.
        std::fs::write(dir.path().join("a.txt"), b"patched build").unwrap();
        let mut m = manifest();
        m.exe_name = "a.txt".to_string();

        // Without the ExePatch size it reads as damage.
        let plain = scan(
            dir.path().to_path_buf(),
            Arc::new(m.clone()),
            ScanMode::Full,
            Arc::new(Control::default()),
            |_| {},
        )
        .await
        .unwrap();
        assert!(plain
            .files
            .iter()
            .any(|f| f.path == "a.txt" && f.is_issue()));

        // With it, the same folder is clean.
        m.exe_patch_size = Some(b"patched build".len() as u64);
        let aware = scan(
            dir.path().to_path_buf(),
            Arc::new(m),
            ScanMode::Full,
            Arc::new(Control::default()),
            |_| {},
        )
        .await
        .unwrap();
        assert!(aware
            .files
            .iter()
            .any(|f| f.path == "a.txt" && !f.is_issue()));
    }

    #[tokio::test]
    async fn a_cancelled_download_does_not_claim_the_files_it_never_fetched() {
        let dir = TempDir::new("mlcancel");
        let cancel: Cancel = Arc::new(Control::default());
        cancel.cancel();

        let report = download(
            dir.path().to_path_buf(),
            Arc::new(manifest()),
            vec!["a.txt".to_string(), "sub/b.bin".to_string()],
            false,
            cancel,
            Arc::new(DownloadProgress::default()),
        )
        .await
        .unwrap();

        assert!(report.cancelled);
        assert_eq!(report.requested, 2);
        // Nothing was fetched, so nothing may be reported as written.
        assert_eq!(report.written, 0);
        assert!(report.failures.is_empty());
    }

    #[tokio::test]
    async fn downloading_a_file_the_manifest_does_not_list_is_refused() {
        let dir = TempDir::new("mldl");
        let err = download(
            dir.path().to_path_buf(),
            Arc::new(manifest()),
            vec!["not-in-manifest.dll".to_string()],
            false,
            Arc::new(Control::default()),
            Arc::new(DownloadProgress::default()),
        )
        .await
        .unwrap_err();
        assert!(err.contains("does not list"));
    }

    const SAMPLE_URL: &str = "http://maplestory-download.beanfun.com/maplestory/productInfo.json";

    const SAMPLE_INFO: &str = r#"{
        "productName": "新楓之谷",
        "version": "V282",
        "publishDate": "2026/09/04",
        "baseUrl": "https://maplestory-download.beanfun.com/maplestory/download/",
        "executionPath": "P2PdPoyK5obH/MapleStory.exe",
        "files": [{"path": "a.wz", "sizeInBytes": 4, "sha256": "ab"}]
    }"#;

    #[test]
    fn a_cached_manifest_round_trips_and_keeps_when_it_was_taken() {
        let dir = TempDir::new("manifest_cache");
        assert!(read_cache(dir.path()).is_none());

        write_cache(dir.path(), SAMPLE_URL, SAMPLE_INFO);
        let cached = read_cache(dir.path()).expect("a copy should have been written");
        assert_eq!(cached.body, SAMPLE_INFO);
        assert_eq!(cached.url, SAMPLE_URL);
        assert!(
            cached.fetched_at.starts_with("20"),
            "not a timestamp: {}",
            cached.fetched_at
        );

        // The cached body is the same text a fresh fetch would have parsed.
        let manifest = parse_manifest(&cached.body).unwrap();
        assert_eq!(manifest.version, "V282");
        assert_eq!(manifest.files.len(), 1);
    }

    #[test]
    fn an_unreadable_cache_reads_as_absent_rather_than_failing() {
        let dir = TempDir::new("manifest_cache_bad");
        std::fs::write(cache_path(dir.path()), b"{ not json").unwrap();
        assert!(read_cache(dir.path()).is_none());
        // And the next good fetch repairs it.
        write_cache(dir.path(), SAMPLE_URL, SAMPLE_INFO);
        assert!(read_cache(dir.path()).is_some());
    }
}

/// Live checks against beanfun. Ignored by default (network + real files);
/// run with `cargo test live_client -- --ignored --nocapture`.
#[cfg(test)]
mod live_tests {
    use super::tests::TempDir;
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn live_client_fetches_and_verifies_two_small_files() {
        let cache = TempDir::new("live_manifest_cache");
        let manifest = Arc::new(fetch_manifest(cache.path()).await.unwrap());
        eprintln!(
            "manifest: {} {} — {} files, {} bytes",
            manifest.product_name, manifest.version, manifest.file_count, manifest.total_bytes
        );

        // Two of the smallest files, so the test is quick but real.
        let mut small: Vec<&ManifestFile> = manifest.files.iter().collect();
        small.sort_by_key(|f| f.size);
        let picked: Vec<String> = small.iter().take(2).map(|f| f.path.clone()).collect();
        eprintln!("picked: {picked:?}");

        let dir = TempDir::new("mllive");
        let cancel: Cancel = Arc::new(Control::default());
        let progress = Arc::new(DownloadProgress::default());

        // An empty folder scans as entirely missing — the full-install path.
        let before = scan(
            dir.path().to_path_buf(),
            manifest.clone(),
            ScanMode::Quick,
            cancel.clone(),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(before.issue_count, manifest.file_count);

        let report = download(
            dir.path().to_path_buf(),
            manifest.clone(),
            picked.clone(),
            false,
            cancel.clone(),
            progress.clone(),
        )
        .await
        .unwrap();
        eprintln!("download: {report:?}");
        assert!(report.failures.is_empty(), "{:?}", report.failures);
        assert_eq!(report.written, 2);

        // Every fetched file now passes a full hash check, and no stray part
        // files are left behind.
        for path in &picked {
            let f = manifest.files.iter().find(|f| &f.path == path).unwrap();
            let target = safe_join(dir.path(), path).unwrap();
            assert_eq!(std::fs::metadata(&target).unwrap().len(), f.size);
            assert!(hash_file(&target).unwrap().eq_ignore_ascii_case(&f.sha256));
        }
        let after = scan(
            dir.path().to_path_buf(),
            manifest.clone(),
            ScanMode::Full,
            cancel,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(after.ok_files, 2);
        assert!(after.extra_files.is_empty(), "{:?}", after.extra_files);
    }

    #[tokio::test]
    #[ignore]
    async fn live_client_leaves_a_good_file_alone_when_a_download_fails() {
        let cache = TempDir::new("live_manifest_cache2");
        let mut manifest = fetch_manifest(cache.path()).await.unwrap();
        let mut small: Vec<ManifestFile> = manifest.files.clone();
        small.sort_by_key(|f| f.size);
        let victim = small[0].clone();

        // Point the manifest at a path that does not exist on the CDN, so the
        // fetch fails after the local file is already in place.
        manifest.folder_name = "definitely-not-a-real-folder".to_string();
        let manifest = Arc::new(manifest);

        let dir = TempDir::new("mllive_fail");
        let target = safe_join(dir.path(), &victim.path).unwrap();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"existing content the player owns").unwrap();

        let report = download(
            dir.path().to_path_buf(),
            manifest,
            vec![victim.path.clone()],
            false,
            Arc::new(Control::default()),
            Arc::new(DownloadProgress::default()),
        )
        .await
        .unwrap();
        assert_eq!(report.written, 0);
        assert_eq!(report.failures.len(), 1);
        eprintln!("expected failure: {:?}", report.failures[0]);

        // Untouched, and nothing half-written left next to it.
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"existing content the player owns"
        );
        let leftovers: Vec<_> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(PART_SUFFIX))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
