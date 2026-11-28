//! Bulk execution engine: safe symlink-confined input discovery, incremental content-hash
//! caching, atomic temp-file staging + rename, bounded parallel execution over single-decode
//! multi-output `v2/execute` DAGs, and structured manifest/snippet emission.

use super::planner::{
    build_still_fanout_envelope, build_video_poster_envelope, build_video_transcode_envelope,
    plan_breakpoints, render_picture_snippet,
};
use super::schema::{
    BulkConfig, BulkManifest, FailurePolicy, MANIFEST_SCHEMA_VERSION, ManifestAsset,
    ManifestVariant, OutputPathLayout, ResponsivePreset,
};
use imageflow_core::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const CACHE_FILE_NAME: &str = ".imageflow-cache.json";
const CACHE_SCHEMA: &str = "imageflow-bulk-cache-v1";
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const SUPPORTED_IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "avif", "gif", "jxl", "bmp"];
const SUPPORTED_VIDEO_EXTS: &[&str] = &["mp4", "webm", "ivf"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredInput {
    pub id: String,
    pub root_dir: PathBuf,
    pub absolute_path: PathBuf,
    pub relative_key: String,
    pub is_video_ext: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkProgressEvent {
    pub index: usize,
    pub total: usize,
    pub relative_key: String,
    pub status: String,
    pub variants_written: usize,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkRunReport {
    pub dry_run: bool,
    pub discovered: usize,
    pub processed: usize,
    pub skipped_cached: usize,
    pub failed: usize,
    pub total_input_bytes: u64,
    pub total_output_bytes: u64,
    pub elapsed_ms: u64,
    pub manifest_path: Option<PathBuf>,
    pub snippets_path: Option<PathBuf>,
    pub errors: Vec<String>,
    pub manifest: BulkManifest,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BulkCacheFile {
    schema: String,
    engine_fingerprint: String,
    entries: BTreeMap<String, CachedAssetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedAssetEntry {
    source_sha256: String,
    recipe_sha256: String,
    asset: ManifestAsset,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn recipe_fingerprint(preset: &ResponsivePreset, layout: OutputPathLayout) -> String {
    let canonical = serde_json::json!({
        "preset": preset,
        "layout": layout,
    });
    sha256_hex(canonical.to_string().as_bytes())
}

fn engine_fingerprint() -> String {
    let caps = Context::create()
        .ok()
        .and_then(|mut ctx| {
            let (resp, res) = ctx.message("v2/capabilities", b"{}");
            res.ok().map(|_| resp.response_json)
        })
        .unwrap_or_default();
    sha256_hex(&caps)[..16].to_string()
}

fn is_supported_ext(path: &Path) -> Option<bool> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if SUPPORTED_IMAGE_EXTS.contains(&ext.as_str()) {
        Some(false)
    } else if SUPPORTED_VIDEO_EXTS.contains(&ext.as_str()) {
        Some(true)
    } else {
        None
    }
}

/// Discover media files deterministically beneath `inputs`, refusing symlinks unless explicitly
/// enabled and skipping any files located inside `output_dir` so re-runs never ingest outputs.
pub fn discover_media_files(
    inputs: &[PathBuf],
    output_dir: Option<&Path>,
    recursive: bool,
    include_globs: &[String],
    exclude_globs: &[String],
    follow_symlinks: bool,
) -> Result<Vec<DiscoveredInput>, String> {
    let include_patterns: Vec<glob::Pattern> = include_globs
        .iter()
        .map(|g| glob::Pattern::new(g).map_err(|e| format!("invalid include glob '{g}': {e}")))
        .collect::<Result<_, _>>()?;
    let exclude_patterns: Vec<glob::Pattern> = exclude_globs
        .iter()
        .map(|g| glob::Pattern::new(g).map_err(|e| format!("invalid exclude glob '{g}': {e}")))
        .collect::<Result<_, _>>()?;

    let canonical_out = output_dir.and_then(|p| fs::canonicalize(p).ok());
    let mut raw_items: Vec<(String, PathBuf, PathBuf, bool, u64)> = Vec::new();
    let mut seen_canon = std::collections::HashSet::new();

    for input in inputs {
        let meta = fs::symlink_metadata(input)
            .map_err(|e| format!("cannot inspect input '{}': {e}", input.display()))?;
        if meta.file_type().is_symlink() && !follow_symlinks {
            continue;
        }
        let canon_root = fs::canonicalize(input)
            .map_err(|e| format!("cannot resolve input '{}': {e}", input.display()))?;

        if canon_root.is_file() {
            if let Some(is_video) = is_supported_ext(&canon_root) {
                if seen_canon.insert(canon_root.clone()) {
                    let rel = canon_root
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("input")
                        .to_string();
                    let root_dir = canon_root.parent().unwrap_or(&canon_root).to_path_buf();
                    raw_items.push((rel, root_dir, canon_root, is_video, meta.len()));
                }
            }
            continue;
        }

        let mut walker = walkdir::WalkDir::new(&canon_root).follow_links(follow_symlinks);
        if !recursive {
            walker = walker.max_depth(1);
        }
        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if entry.file_type().is_symlink() && !follow_symlinks {
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            if entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with('.'))
            {
                continue;
            }
            let Some(is_video) = is_supported_ext(path) else {
                continue;
            };
            let Ok(canon_file) = fs::canonicalize(path) else {
                continue;
            };
            if !follow_symlinks && !canon_file.starts_with(&canon_root) {
                continue;
            }
            if let Some(ref co) = canonical_out
                && canon_file.starts_with(co)
            {
                continue;
            }
            let rel_raw = canon_file
                .strip_prefix(&canon_root)
                .unwrap_or(&canon_file)
                .to_string_lossy();
            #[cfg(windows)]
            let rel_path = rel_raw.replace('\\', "/");
            #[cfg(not(windows))]
            let rel_path = rel_raw.into_owned();
            if !include_patterns.is_empty()
                && !include_patterns.iter().any(|p| p.matches(&rel_path))
            {
                continue;
            }
            if exclude_patterns.iter().any(|p| p.matches(&rel_path)) {
                continue;
            }
            if seen_canon.insert(canon_file.clone()) {
                let size = entry.metadata().map_or(0, |m| m.len());
                raw_items.push((rel_path, canon_root.clone(), canon_file, is_video, size));
            }
        }
    }

    raw_items.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
    let items = raw_items
        .into_iter()
        .enumerate()
        .map(|(i, (relative_key, root_dir, absolute_path, is_video_ext, size_bytes))| {
            DiscoveredInput {
                id: format!("a{:04}", i + 1),
                root_dir,
                absolute_path,
                relative_key,
                is_video_ext,
                size_bytes,
            }
        })
        .collect();
    Ok(items)
}

/// Generate a 128-bit cryptographically secure random hex string using the OS CSPRNG (fails closed).
pub fn csprng_hex_128() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("OS CSPRNG unavailable: {e}"))?;
    Ok(hex::encode(bytes))
}

/// Verify that `target` stays strictly inside `root` with zero symlink components along its path.
pub fn verify_confined_no_symlinks(root: &Path, target: &Path) -> Result<(), String> {
    let root_meta = fs::symlink_metadata(root)
        .map_err(|e| format!("cannot inspect root '{}': {e}", root.display()))?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(format!("root '{}' must be a real directory (no symlink)", root.display()));
    }
    let rel = target.strip_prefix(root).map_err(|_| {
        format!("path '{}' escapes root '{}'", target.display(), root.display())
    })?;
    let mut cur = root.to_path_buf();
    for comp in rel.components() {
        match comp {
            std::path::Component::Normal(seg) => {
                cur.push(seg);
                match fs::symlink_metadata(&cur) {
                    Ok(m) => {
                        if m.file_type().is_symlink() {
                            return Err(format!(
                                "refusing symlink component '{}' inside confined root '{}'",
                                cur.display(),
                                root.display()
                            ));
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        return Err(format!("metadata check failed for '{}': {e}", cur.display()));
                    }
                }
            }
            std::path::Component::CurDir => {}
            _ => {
                return Err(format!(
                    "disallowed path component in '{}' relative to '{}'",
                    target.display(),
                    root.display()
                ));
            }
        }
    }
    Ok(())
}

/// Create all ancestor directories beneath `root` without ever traversing a symlink.
#[cfg(not(unix))]
fn create_confined_dir_all(root: &Path, dir: &Path) -> Result<(), String> {
    verify_confined_no_symlinks(root, dir)?;
    let rel = dir.strip_prefix(root).map_err(|_| {
        format!("directory '{}' escapes root '{}'", dir.display(), root.display())
    })?;
    let mut cur = root.to_path_buf();
    for comp in rel.components() {
        if let std::path::Component::Normal(seg) = comp {
            cur.push(seg);
            match fs::symlink_metadata(&cur) {
                Ok(m) => {
                    if m.file_type().is_symlink() || !m.is_dir() {
                        return Err(format!(
                            "refusing non-directory or symlink path component '{}'",
                            cur.display()
                        ));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&cur)
                        .map_err(|ce| format!("failed creating '{}': {ce}", cur.display()))?;
                }
                Err(e) => return Err(format!("metadata check failed for '{}': {e}", cur.display())),
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn open_confined_dir_fd(
    root: &Path,
    rel_dir: &Path,
    create_missing: bool,
) -> Result<rustix::fd::OwnedFd, String> {
    use rustix::fs::{CWD, Mode, OFlags, mkdirat, openat};
    let dir_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut cur = openat(CWD, root, dir_flags, Mode::empty())
        .map_err(|e| format!("failed opening confined root '{}': {e}", root.display()))?;
    for comp in rel_dir.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(seg) => match openat(&cur, seg, dir_flags, Mode::empty()) {
                Ok(next) => cur = next,
                Err(e) if create_missing && e == rustix::io::Errno::NOENT => {
                    mkdirat(&cur, seg, Mode::RWXU | Mode::RGRP | Mode::XGRP | Mode::ROTH | Mode::XOTH)
                        .or_else(|me| {
                            if me == rustix::io::Errno::EXIST {
                                Ok(())
                            } else {
                                Err(me)
                            }
                        })
                        .map_err(|me| {
                            format!("failed creating confined dir '{}': {me}", seg.to_string_lossy())
                        })?;
                    cur = openat(&cur, seg, dir_flags, Mode::empty()).map_err(|oe| {
                        format!(
                            "refusing symlink or non-directory component '{}': {oe}",
                            seg.to_string_lossy()
                        )
                    })?;
                }
                Err(e) => {
                    return Err(format!(
                        "refusing symlink or invalid path component '{}': {e}",
                        seg.to_string_lossy()
                    ));
                }
            },
            _ => {
                return Err(format!(
                    "disallowed path component in confined relative dir '{}'",
                    rel_dir.display()
                ));
            }
        }
    }
    Ok(cur)
}

/// Open and read a regular source file strictly confined beneath `root` using directory-handle-
/// anchored `openat(..., O_NOFOLLOW | O_DIRECTORY)` traversal on Unix (zero parent-swap TOCTOU).
pub fn read_confined_regular_file(root: &Path, path: &Path) -> Result<Vec<u8>, String> {
    #[cfg(unix)]
    {
        use rustix::fs::{FileType, Mode, OFlags, fstat, openat};
        let rel = path.strip_prefix(root).map_err(|_| {
            format!("path '{}' escapes root '{}'", path.display(), root.display())
        })?;
        let file_name = rel
            .file_name()
            .ok_or_else(|| format!("confined file '{}' has no filename", path.display()))?;
        let parent_rel = rel.parent().unwrap_or_else(|| Path::new(""));
        let dir_fd = open_confined_dir_fd(root, parent_rel, false)?;
        let fd = openat(
            &dir_fd,
            file_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| format!("failed opening confined file '{}': {e}", path.display()))?;
        let st = fstat(&fd)
            .map_err(|e| format!("failed stat on confined file '{}': {e}", path.display()))?;
        if FileType::from_raw_mode(st.st_mode as _) != FileType::RegularFile {
            return Err(format!("refusing non-regular file '{}'", path.display()));
        }
        let mut f = fs::File::from(fd);
        let mut data = Vec::with_capacity((st.st_size.max(0) as usize).min(64 * 1024 * 1024));
        std::io::Read::read_to_end(&mut f, &mut data)
            .map_err(|e| format!("failed reading confined file '{}': {e}", path.display()))?;
        Ok(data)
    }
    #[cfg(not(unix))]
    {
        verify_confined_no_symlinks(root, path)?;
        let mut f = fs::File::open(path)
            .map_err(|e| format!("failed opening confined file '{}': {e}", path.display()))?;
        let meta = f
            .metadata()
            .map_err(|e| format!("failed stat on confined file '{}': {e}", path.display()))?;
        if !meta.is_file() {
            return Err(format!("refusing non-regular file '{}'", path.display()));
        }
        let mut data = Vec::with_capacity(meta.len().min(64 * 1024 * 1024) as usize);
        std::io::Read::read_to_end(&mut f, &mut data)
            .map_err(|e| format!("failed reading confined file '{}': {e}", path.display()))?;
        Ok(data)
    }
}

/// Validate a cached or emitted variant relative path so cached manifests can never inject
/// traversal, scheme URLs, or unsafe characters into HTML snippets.
pub fn is_safe_variant_relative_path(rel: &str) -> bool {
    if !rel.starts_with("assets/") || rel.contains("..") || rel.contains('\\') || rel.contains(':') {
        return false;
    }
    rel.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.')
    })
}

/// One file already staged to disk as an exclusive sibling temp file beneath its confined root.
#[derive(Debug)]
struct StagedDiskFile {
    parent_rel: PathBuf,
    tmp_name: String,
    dest_name: std::ffi::OsString,
}

/// Disk-backed batch of staged output files with automatic temp cleanup on error/drop.
/// Retains ZERO encoded image bytes in RAM and ZERO open file/directory descriptors across a batch!
#[derive(Debug)]
struct StagedDiskBatch {
    root: PathBuf,
    entries: Vec<StagedDiskFile>,
}

impl StagedDiskBatch {
    fn new(root: PathBuf) -> Self {
        Self { root, entries: Vec::new() }
    }

    fn extend_from(&mut self, mut other: StagedDiskBatch) {
        self.entries.append(&mut other.entries);
    }
}

impl Drop for StagedDiskBatch {
    fn drop(&mut self) {
        for entry in &self.entries {
            unlink_confined_entry(&self.root, &entry.parent_rel, &entry.tmp_name);
        }
    }
}

fn unlink_confined_entry(root: &Path, parent_rel: &Path, file_name: &str) {
    #[cfg(unix)]
    {
        use rustix::fs::{AtFlags, unlinkat};
        if let Ok(dir_fd) = open_confined_dir_fd(root, parent_rel, false) {
            let _ = unlinkat(&dir_fd, file_name, AtFlags::empty());
        }
    }
    #[cfg(not(unix))]
    {
        let p = if parent_rel.as_os_str().is_empty() {
            root.join(file_name)
        } else {
            root.join(parent_rel).join(file_name)
        };
        let _ = fs::remove_file(p);
    }
}

/// Write `bytes` atomically to `dest` confined beneath `root`, refusing symlinks and creating the
/// staging file exclusively (`O_CREAT | O_EXCL`) anchored to the verified directory handle.
pub fn write_file_atomic_confined(root: &Path, dest: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_staged_files_atomic(root, &[(dest.to_path_buf(), bytes.to_vec())])
}

/// Stage `files` immediately to exclusive sibling temp files on disk beneath `root`, releasing all
/// file/directory descriptors and in-memory buffers as each file finishes.
fn stage_files_to_disk(root: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<StagedDiskBatch, String> {
    let mut batch = StagedDiskBatch::new(root.to_path_buf());
    for (dest, bytes) in files {
        let rel = dest.strip_prefix(root).map_err(|_| {
            format!("destination '{}' escapes root '{}'", dest.display(), root.display())
        })?;
        let dest_name = rel
            .file_name()
            .ok_or_else(|| format!("destination '{}' has no filename", dest.display()))?
            .to_os_string();
        let parent_rel = rel.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nonce = csprng_hex_128()?;
        let tmp_name = format!(".if-stage-{}-{}-{seq}.tmp", std::process::id(), &nonce[..12]);

        #[cfg(unix)]
        {
            use rustix::fs::{AtFlags, Mode, OFlags, openat, unlinkat};
            let dir_fd = open_confined_dir_fd(root, &parent_rel, true)?;
            let write_res = (|| -> Result<(), String> {
                let fd = openat(
                    &dir_fd,
                    &tmp_name,
                    OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
                )
                .map_err(|e| format!("failed creating confined temp '{}': {e}", tmp_name))?;
                let mut f = fs::File::from(fd);
                f.write_all(bytes)
                    .map_err(|e| format!("failed writing temp '{}': {e}", tmp_name))?;
                f.sync_all()
                    .map_err(|e| format!("failed syncing temp '{}': {e}", tmp_name))?;
                Ok(())
            })();
            if let Err(e) = write_res {
                let _ = unlinkat(&dir_fd, &tmp_name, AtFlags::empty());
                return Err(e);
            }
            // `dir_fd` drops right here — O(1) open directory descriptors!
        }
        #[cfg(not(unix))]
        {
            let parent = dest
                .parent()
                .ok_or_else(|| format!("destination '{}' has no parent directory", dest.display()))?;
            create_confined_dir_all(root, parent)?;
            verify_confined_no_symlinks(root, dest)?;
            let tmp_path = parent.join(&tmp_name);
            let write_res = (|| -> std::io::Result<()> {
                let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp_path)?;
                f.write_all(bytes)?;
                f.sync_all()?;
                Ok(())
            })();
            if let Err(e) = write_res {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("failed staging temp file '{}': {e}", tmp_path.display()));
            }
        }
        batch.entries.push(StagedDiskFile { parent_rel, tmp_name, dest_name });
    }
    Ok(batch)
}

/// Commit a disk-staged batch atomically with O(1) open directory descriptors, ZERO missing-
/// entrypoint window, and exact rollback of every earlier replacement if a late rename fails.
fn commit_staged_batch(batch: &mut StagedDiskBatch) -> Result<(), String> {
    if batch.entries.is_empty() {
        return Ok(());
    }
    let root = batch.root.clone();
    let entries = std::mem::take(&mut batch.entries);
    let mut backups: Vec<Option<String>> = vec![None; entries.len()];

    // Helper to clean up remaining backups + remaining temp files on early error.
    // Entries whose backup slot was cleared (already restored OR preserved after a failed restore)
    // are intentionally skipped so recovery copies are NEVER deleted!
    let cleanup_remaining = |entries: &[StagedDiskFile], backups: &[Option<String>], start_tmp: usize| {
        for (idx, e) in entries.iter().enumerate() {
            if let Some(ref bak) = backups[idx] {
                unlink_confined_entry(&root, &e.parent_rel, bak);
            }
            if idx >= start_tmp {
                unlink_confined_entry(&root, &e.parent_rel, &e.tmp_name);
            }
        }
    };

    // Phase 1: pre-check EVERY destination (refusing directories/symlinks) and snapshot existing
    // regular files via sibling hardlinks WITHOUT moving live files aside (zero missing window!).
    for (idx, entry) in entries.iter().enumerate() {
        let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nonce = match csprng_hex_128() {
            Ok(n) => n,
            Err(e) => {
                cleanup_remaining(&entries, &backups, 0);
                return Err(e);
            }
        };
        let bak_name = format!(".if-bak-{}-{}-{seq}.tmp", std::process::id(), &nonce[..12]);

        #[cfg(unix)]
        let bak_res: Result<bool, String> = (|| {
            use rustix::fs::{AtFlags, FileType, Mode, OFlags, linkat, openat, statat, unlinkat};
            let dir_fd = open_confined_dir_fd(&root, &entry.parent_rel, false)?;
            match statat(&dir_fd, &entry.dest_name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(st) => {
                    if FileType::from_raw_mode(st.st_mode as _) != FileType::RegularFile {
                        return Err(format!(
                            "refusing to overwrite non-regular destination '{}'",
                            entry.dest_name.to_string_lossy()
                        ));
                    }
                    if linkat(&dir_fd, &entry.dest_name, &dir_fd, &bak_name, AtFlags::empty()).is_err() {
                        // Fallback when filesystem forbids hardlinks: copy inside the same verified dir FD.
                        let src_fd = openat(
                            &dir_fd,
                            &entry.dest_name,
                            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                            Mode::empty(),
                        )
                        .map_err(|e| format!("failed opening destination for backup: {e}"))?;
                        let dst_fd = openat(
                            &dir_fd,
                            &bak_name,
                            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                            Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
                        )
                        .map_err(|e| format!("failed creating rollback backup '{}': {e}", bak_name))?;
                        let mut src_f = fs::File::from(src_fd);
                        let mut dst_f = fs::File::from(dst_fd);
                        if let Err(e) = std::io::copy(&mut src_f, &mut dst_f) {
                            let _ = unlinkat(&dir_fd, &bak_name, AtFlags::empty());
                            return Err(format!("failed copying rollback backup: {e}"));
                        }
                        let _ = dst_f.sync_all();
                    }
                    Ok(true)
                }
                Err(rustix::io::Errno::NOENT) => Ok(false),
                Err(e) => Err(format!(
                    "failed inspecting destination '{}': {e}",
                    entry.dest_name.to_string_lossy()
                )),
            }
        })();

        #[cfg(not(unix))]
        let bak_res: Result<bool, String> = (|| {
            let dir = if entry.parent_rel.as_os_str().is_empty() {
                root.clone()
            } else {
                root.join(&entry.parent_rel)
            };
            let dest_path = dir.join(&entry.dest_name);
            verify_confined_no_symlinks(&root, &dest_path)?;
            match fs::symlink_metadata(&dest_path) {
                Ok(m) => {
                    if m.file_type().is_symlink() || !m.is_file() {
                        return Err(format!(
                            "refusing to overwrite non-regular destination '{}'",
                            dest_path.display()
                        ));
                    }
                    let bak_path = dir.join(&bak_name);
                    if fs::hard_link(&dest_path, &bak_path).is_err() {
                        fs::copy(&dest_path, &bak_path)
                            .map_err(|e| format!("failed creating rollback backup: {e}"))?;
                    }
                    Ok(true)
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(format!("failed inspecting '{}': {e}", dest_path.display())),
            }
        })();

        match bak_res {
            Ok(true) => backups[idx] = Some(bak_name),
            Ok(false) => {}
            Err(e) => {
                cleanup_remaining(&entries, &backups, 0);
                return Err(e);
            }
        }
    }

    // Phase 2: single atomic rename per file in order (entrypoint last; zero missing window!).
    for (idx, entry) in entries.iter().enumerate() {
        #[cfg(unix)]
        let swap_res: Result<(), String> = (|| {
            use rustix::fs::renameat;
            let dir_fd = open_confined_dir_fd(&root, &entry.parent_rel, false)?;
            renameat(&dir_fd, &entry.tmp_name, &dir_fd, &entry.dest_name).map_err(|e| {
                format!(
                    "failed atomic rename to '{}': {e}",
                    entry.dest_name.to_string_lossy()
                )
            })
        })();

        #[cfg(not(unix))]
        let swap_res: Result<(), String> = (|| {
            let dir = if entry.parent_rel.as_os_str().is_empty() {
                root.clone()
            } else {
                root.join(&entry.parent_rel)
            };
            let tmp_path = dir.join(&entry.tmp_name);
            let dest_path = dir.join(&entry.dest_name);
            verify_confined_no_symlinks(&root, &dest_path)?;
            fs::rename(&tmp_path, &dest_path)
                .map_err(|e| format!("failed atomic rename to '{}': {e}", dest_path.display()))
        })();

        if let Err(err) = swap_res {
            // Roll back EVERY already-replaced file in reverse order from its sibling backup!
            let mut restore_errs = Vec::new();
            for prev in (0..idx).rev() {
                let pe = &entries[prev];
                #[cfg(unix)]
                let rres: Result<(), String> = (|| {
                    use rustix::fs::{AtFlags, renameat, unlinkat};
                    let dir_fd = open_confined_dir_fd(&root, &pe.parent_rel, false)?;
                    if let Some(ref bak) = backups[prev] {
                        renameat(&dir_fd, bak, &dir_fd, &pe.dest_name)
                            .map_err(|e| format!("restore '{}' failed: {e}", pe.dest_name.to_string_lossy()))
                    } else {
                        unlinkat(&dir_fd, &pe.dest_name, AtFlags::empty())
                            .or_else(|e| if e == rustix::io::Errno::NOENT { Ok(()) } else { Err(e) })
                            .map_err(|e| format!("unlink '{}' failed: {e}", pe.dest_name.to_string_lossy()))
                    }
                })();
                #[cfg(not(unix))]
                let rres: Result<(), String> = (|| {
                    let dir = if pe.parent_rel.as_os_str().is_empty() {
                        root.clone()
                    } else {
                        root.join(&pe.parent_rel)
                    };
                    let dest_path = dir.join(&pe.dest_name);
                    if let Some(ref bak) = backups[prev] {
                        fs::rename(dir.join(bak), &dest_path)
                            .map_err(|e| format!("restore '{}' failed: {e}", dest_path.display()))
                    } else {
                        let _ = fs::remove_file(&dest_path);
                        Ok(())
                    }
                })();
                match rres {
                    Ok(()) => backups[prev] = None,
                    Err(re) => {
                        // NEVER delete the recovery copy when restore fails! Take ownership out of
                        // `backups[prev]` so `cleanup_remaining` leaves it intact and report its path.
                        if let Some(preserved_bak) = backups[prev].take() {
                            let bak_path = if pe.parent_rel.as_os_str().is_empty() {
                                root.join(&preserved_bak)
                            } else {
                                root.join(&pe.parent_rel).join(&preserved_bak)
                            };
                            restore_errs.push(format!(
                                "{re} (preserved recovery copy at '{}')",
                                bak_path.display()
                            ));
                        } else {
                            restore_errs.push(re);
                        }
                    }
                }
            }
            cleanup_remaining(&entries, &backups, idx);
            if !restore_errs.is_empty() {
                return Err(format!(
                    "{err}; rollback also failed for: {}",
                    restore_errs.join("; ")
                ));
            }
            return Err(err);
        }
    }

    // Phase 3: unlink all sibling backup hardlinks once the entire batch succeeded.
    for (idx, entry) in entries.iter().enumerate() {
        if let Some(ref bak) = backups[idx] {
            unlink_confined_entry(&root, &entry.parent_rel, bak);
        }
    }
    Ok(())
}

/// Stage ALL `(dest, bytes)` files exclusively into sibling temp files anchored beneath `root`,
/// then atomically replace each destination with O(1) open directory FDs, ZERO missing-entrypoint
/// window, and exact rollback of earlier replacements if a late rename fails.
pub fn publish_staged_files_atomic(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }
    let mut batch = stage_files_to_disk(root, files)?;
    commit_staged_batch(&mut batch)
}

/// Write `bytes` atomically to `dest` via an exclusive sibling temp file + rename.
pub fn write_file_atomic(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = dest
        .parent()
        .ok_or_else(|| format!("destination '{}' has no parent directory", dest.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create directory '{}': {e}", parent.display()))?;
    write_file_atomic_confined(parent, dest, bytes)
}

fn sanitize_stem(rel_key: &str) -> String {
    let p = Path::new(rel_key);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("asset");
    let mut clean: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if clean.is_empty() {
        clean.push_str("asset");
    }
    if clean.len() > 64 {
        clean.truncate(64);
    }
    clean
}

fn sanitize_segment(seg: &str) -> String {
    let mut clean: String = seg
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if clean.is_empty() || clean == "." || clean == ".." {
        clean = "dir".to_string();
    }
    if clean.len() > 64 {
        clean.truncate(64);
    }
    clean
}

fn variant_relative_path(
    layout: OutputPathLayout,
    rel_key: &str,
    width: u32,
    height: u32,
    ext: &str,
    content_hash8: &str,
) -> String {
    let stem = sanitize_stem(rel_key);
    match layout {
        OutputPathLayout::ContentAddressed => {
            format!("assets/{stem}-{width}x{height}-{content_hash8}.{ext}")
        }
        OutputPathLayout::Flatten => {
            format!("assets/{stem}-{width}x{height}.{ext}")
        }
        OutputPathLayout::PreserveTree => {
            let safe_parents: Vec<String> = Path::new(rel_key)
                .parent()
                .map(|p| {
                    p.components()
                        .filter_map(|c| match c {
                            std::path::Component::Normal(s) => Some(sanitize_segment(&s.to_string_lossy())),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            if safe_parents.is_empty() {
                format!("assets/{stem}-{width}x{height}.{ext}")
            } else {
                format!("assets/{}/{stem}-{width}x{height}.{ext}", safe_parents.join("/"))
            }
        }
    }
}

fn verify_cached_entry_on_disk(entry: &CachedAssetEntry, output_dir: &Path) -> bool {
    let all_variants = entry.asset.variants.iter().chain(entry.asset.video_variants.iter());
    for v in all_variants {
        if !is_safe_variant_relative_path(&v.relative_path) {
            return false;
        }
        let full = output_dir.join(&v.relative_path);
        if verify_confined_no_symlinks(output_dir, &full).is_err() {
            return false;
        }
        let Ok(meta) = fs::symlink_metadata(&full) else {
            return false;
        };
        if !meta.is_file() || meta.len() != v.bytes {
            return false;
        }
    }
    !entry.asset.variants.is_empty()
}

fn inspect_input_bytes(
    bytes: &[u8],
    is_video_ext: bool,
    workspace_policy: &str,
    limits: &super::schema::BulkLimits,
) -> Result<(u32, u32, String, String), String> {
    let mut ctx = Context::create().map_err(|e| format!("context create failed: {e}"))?;
    let cfg = serde_json::json!({
        "workspace_policy": workspace_policy,
        "limits": {
            "max_memory_bytes": limits.max_memory_bytes,
            "timeout_ms": limits.timeout_ms,
            "max_dimension": limits.max_dimension
        }
    });
    ctx.message("v2/configure_context", cfg.to_string().as_bytes())
        .1
        .map_err(|e| format!("configure_context failed: {e}"))?;
    ctx.add_copied_input_buffer(0, bytes)
        .map_err(|e| format!("add input buffer failed: {e}"))?;
    let insp_req = if is_video_ext {
        let container = if bytes.len() >= 4 && &bytes[0..4] == b"\x1A\x45\xDF\xA3" {
            "webm"
        } else if bytes.len() >= 4 && &bytes[0..4] == b"DKIF" {
            "ivf"
        } else {
            "mp4"
        };
        serde_json::json!({
            "inputs": { "src": 0 },
            "input": "src",
            "video": { "container": container },
            "scan": "headers"
        })
    } else {
        serde_json::json!({
            "inputs": { "src": 0 },
            "input": "src",
            "scan": "headers"
        })
    };
    let (resp, res) = ctx.message("v2/inspect", insp_req.to_string().as_bytes());
    res.map_err(|e| format!("v2/inspect failed: {e}"))?;
    let body: serde_json::Value = serde_json::from_slice(&resp.response_json)
        .map_err(|e| format!("invalid inspect JSON: {e}"))?;
    let data = &body["data"];
    let container = data["container"].as_str().unwrap_or("unknown").to_string();
    let track = data["tracks"]
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|t| matches!(t["kind"].as_str(), Some("video" | "image" | "animation")))
                .or_else(|| arr.first())
        })
        .ok_or_else(|| "v2/inspect returned no visual track".to_string())?;
    let kind = track["kind"].as_str().unwrap_or("image").to_string();
    let dims = track.get("display").and_then(|d| d.as_object()).or_else(|| {
        track.get("coded").and_then(|d| d.as_object())
    });
    let w = dims.and_then(|d| d.get("width")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let h = dims.and_then(|d| d.get("height")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if w == 0 || h == 0 {
        return Err("v2/inspect reported zero dimensions".into());
    }
    Ok((w, h, kind, container))
}

fn process_single_input(
    item: &DiscoveredInput,
    config: &BulkConfig,
    recipe_hash: &str,
    cached: Option<&CachedAssetEntry>,
    defer_publish: bool,
) -> Result<(ManifestAsset, bool, StagedDiskBatch), String> {
    let source_bytes = if config.follow_symlinks {
        fs::read(&item.absolute_path)
            .map_err(|e| format!("failed reading '{}': {e}", item.relative_key))?
    } else {
        read_confined_regular_file(&item.root_dir, &item.absolute_path)?
    };
    let source_sha = sha256_hex(&source_bytes);

    if config.incremental_cache
        && !config.force_rebuild
        && let Some(c) = cached
        && c.source_sha256 == source_sha
        && c.recipe_sha256 == recipe_hash
        && verify_cached_entry_on_disk(c, &config.output_dir)
    {
        let mut reused = c.asset.clone();
        reused.id = item.id.clone();
        // Always regenerate picture_html from validated, confined variant paths (never trust cached HTML).
        let default_alt = Path::new(&item.relative_key)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace(['-', '_'], " ");
        reused.picture_html =
            render_picture_snippet(&reused, "", "(max-width: 768px) 100vw, 50vw", &default_alt, false);
        return Ok((reused, true, StagedDiskBatch::new(config.output_dir.clone())));
    }

    let (src_w, src_h, media_kind, source_format) = inspect_input_bytes(
        &source_bytes,
        item.is_video_ext,
        config.preset.workspace_policy.as_wire_str(),
        &config.limits,
    )?;

    let is_sequence = media_kind == "video" || media_kind == "animation";
    let breakpoints = plan_breakpoints(src_w, src_h, &config.preset);

    let (envelope, planned_slots) = if is_sequence {
        build_video_poster_envelope(&breakpoints, &config.preset, &config.limits)?
    } else {
        build_still_fanout_envelope(&breakpoints, &config.preset, &config.limits)?
    };

    let mut ctx = Context::create().map_err(|e| format!("context create failed: {e}"))?;
    let ctx_cfg = serde_json::json!({
        "workspace_policy": config.preset.workspace_policy.as_wire_str(),
        "limits": {
            "max_memory_bytes": config.limits.max_memory_bytes,
            "timeout_ms": config.limits.timeout_ms,
            "max_dimension": config.limits.max_dimension
        }
    });
    ctx.message("v2/configure_context", ctx_cfg.to_string().as_bytes())
        .1
        .map_err(|e| format!("configure_context failed: {e}"))?;
    ctx.add_copied_input_buffer(0, &source_bytes)
        .map_err(|e| format!("add_copied_input_buffer failed: {e}"))?;
    for slot in &planned_slots {
        ctx.add_output_buffer(slot.io_id)
            .map_err(|e| format!("add_output_buffer({}) failed: {e}", slot.io_id))?;
    }

    let (resp, res) = ctx.message("v2/execute", envelope.to_string().as_bytes());
    res.map_err(|e| format!("v2/execute failed on '{}': {e}", item.relative_key))?;
    let resp_val: serde_json::Value = serde_json::from_slice(&resp.response_json)
        .map_err(|e| format!("invalid v2/execute JSON: {e}"))?;
    let out_arr = resp_val["data"]["outputs"]
        .as_array()
        .ok_or_else(|| "v2/execute missing data.outputs array".to_string())?;

    // Collect all encoded outputs in memory first; only publish atomically once all succeed.
    let mut staged: Vec<(PathBuf, Vec<u8>, ManifestVariant)> =
        Vec::with_capacity(planned_slots.len());
    for slot in &planned_slots {
        let bytes = ctx
            .take_output_buffer(slot.io_id)
            .map_err(|e| format!("take_output_buffer({}) failed: {e}", slot.io_id))?;
        let out_meta = out_arr
            .iter()
            .find(|o| o["slot"].as_str() == Some(slot.slot_name.as_str()));
        let actual_w = out_meta
            .and_then(|o| o["width"].as_u64())
            .map(|n| n as u32)
            .filter(|&n| n > 0)
            .unwrap_or(slot.breakpoint.width);
        let actual_h = out_meta
            .and_then(|o| o["height"].as_u64())
            .map(|n| n as u32)
            .filter(|&n| n > 0)
            .unwrap_or(slot.breakpoint.height);
        let verified_metric = out_meta
            .and_then(|o| o["verified_metric"].as_str())
            .map(ToString::to_string);
        let verified_score = out_meta.and_then(|o| o["verified_score_f64"].as_f64());

        let hash8 = &sha256_hex(&bytes)[..8];
        let rel_path = variant_relative_path(
            config.output_layout,
            &item.relative_key,
            actual_w,
            actual_h,
            slot.format.extension(),
            hash8,
        );
        let full_path = config.output_dir.join(&rel_path);
        let variant = ManifestVariant {
            relative_path: rel_path,
            format: slot.format,
            mime_type: slot.format.mime_type().to_string(),
            width: actual_w,
            height: actual_h,
            bytes: bytes.len() as u64,
            requested_coding: slot.resolved_coding.requested_label.clone(),
            effective_coding: slot.resolved_coding.effective_label.clone(),
            fallback_reason: slot.resolved_coding.fallback_reason.clone(),
            verified_metric,
            verified_score,
        };
        staged.push((full_path, bytes, variant));
    }

    // Optional video playback variants for video/animation inputs.
    let mut staged_videos: Vec<(PathBuf, Vec<u8>, ManifestVariant)> = Vec::new();
    if is_sequence && !config.preset.video_formats.is_empty() {
        let max_w = config.preset.widths_px.iter().copied().max();
        for &vfmt in &config.preset.video_formats {
            let v_env = build_video_transcode_envelope(vfmt, max_w, &config.limits)?;
            let mut vctx = Context::create().map_err(|e| format!("video ctx failed: {e}"))?;
            vctx.message("v2/configure_context", ctx_cfg.to_string().as_bytes())
                .1
                .map_err(|e| format!("video configure_context failed: {e}"))?;
            vctx.add_copied_input_buffer(0, &source_bytes)
                .map_err(|e| format!("video input buffer failed: {e}"))?;
            vctx.add_output_buffer(1)
                .map_err(|e| format!("video output buffer failed: {e}"))?;
            let (vresp, vres) = vctx.message("v2/execute", v_env.to_string().as_bytes());
            vres.map_err(|e| {
                format!("video transcode ({vfmt}) failed on '{}': {e}", item.relative_key)
            })?;
            let vbytes = vctx
                .take_output_buffer(1)
                .map_err(|e| format!("take video output failed: {e}"))?;
            let vjson: serde_json::Value =
                serde_json::from_slice(&vresp.response_json).unwrap_or_default();
            let vw = vjson["data"]["outputs"][0]["width"]
                .as_u64()
                .unwrap_or(u64::from(src_w)) as u32;
            let vh = vjson["data"]["outputs"][0]["height"]
                .as_u64()
                .unwrap_or(u64::from(src_h)) as u32;
            let hash8 = &sha256_hex(&vbytes)[..8];
            let rel = variant_relative_path(
                config.output_layout,
                &item.relative_key,
                vw,
                vh,
                vfmt.extension(),
                hash8,
            );
            let full = config.output_dir.join(&rel);
            let vlen = vbytes.len() as u64;
            staged_videos.push((
                full,
                vbytes,
                ManifestVariant {
                    relative_path: rel,
                    format: vfmt,
                    mime_type: vfmt.mime_type().to_string(),
                    width: vw,
                    height: vh,
                    bytes: vlen,
                    requested_coding: "video_default".to_string(),
                    effective_coding: format!(
                        "{}/{}",
                        vfmt.wire_format(),
                        vfmt.video_codec().unwrap_or("")
                    ),
                    fallback_reason: None,
                    verified_metric: None,
                    verified_score: None,
                },
            ));
        }
    }

    let mut all_files: Vec<(PathBuf, Vec<u8>)> =
        Vec::with_capacity(staged.len() + staged_videos.len());
    let mut variants = Vec::with_capacity(staged.len());
    for (dest, bytes, var) in staged {
        all_files.push((dest, bytes));
        variants.push(var);
    }
    let mut video_variants = Vec::with_capacity(staged_videos.len());
    for (dest, bytes, var) in staged_videos {
        all_files.push((dest, bytes));
        video_variants.push(var);
    }
    // Stage bytes immediately to disk so deferred FailFast batches never hold encoded image
    // buffers in RAM or open directory descriptors across the batch!
    let mut staged_batch = stage_files_to_disk(&config.output_dir, &all_files)?;
    drop(all_files);
    if !defer_publish {
        commit_staged_batch(&mut staged_batch)?;
    }

    let mut asset = ManifestAsset {
        id: item.id.clone(),
        source_relative_path: item.relative_key.clone(),
        source_sha256: source_sha,
        source_bytes: source_bytes.len() as u64,
        media_kind,
        source_format,
        original_width: src_w,
        original_height: src_h,
        variants,
        video_variants,
        picture_html: String::new(),
    };
    let default_alt = Path::new(&item.relative_key)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .replace(['-', '_'], " ");
    asset.picture_html =
        render_picture_snippet(&asset, "", "(max-width: 768px) 100vw, 50vw", &default_alt, false);

    Ok((asset, false, staged_batch))
}

/// Dry-run bulk planner: discovers inputs and validates routes without writing any files.
pub fn plan_bulk_dry_run(config: &BulkConfig) -> Result<BulkRunReport, String> {
    config.validate()?;
    // Validate route coding resolution up front (e.g. StrictVerify catches unsupported formats immediately).
    for &fmt in &config.preset.formats {
        let _ = super::planner::resolve_variant_coding(
            fmt,
            &config.preset.quality,
            config.preset.quality_fallback,
            config.preset.workspace_policy,
        )?;
    }
    let discovered = discover_media_files(
        &config.inputs,
        Some(&config.output_dir),
        config.recursive,
        &config.include_globs,
        &config.exclude_globs,
        config.follow_symlinks,
    )?;
    let total_in: u64 = discovered.iter().map(|d| d.size_bytes).sum();
    let manifest = BulkManifest {
        schema: MANIFEST_SCHEMA_VERSION.to_string(),
        generated_at_utc: now_utc_iso(),
        generator: format!("imageflow_tool {}", imageflow_types::version::one_line_version()),
        preset: config.preset.clone(),
        total_input_bytes: total_in,
        total_output_bytes: 0,
        assets: Vec::new(),
    };
    Ok(BulkRunReport {
        dry_run: true,
        discovered: discovered.len(),
        processed: 0,
        skipped_cached: 0,
        failed: 0,
        total_input_bytes: total_in,
        total_output_bytes: 0,
        elapsed_ms: 0,
        manifest_path: None,
        snippets_path: None,
        errors: Vec::new(),
        manifest,
    })
}

/// Execute a bulk processing job with incremental caching, bounded parallelism, atomic writes,
/// and optional manifest + `<picture>` snippets output.
pub fn run_bulk(
    config: &BulkConfig,
    cancel: Option<Arc<AtomicBool>>,
    on_progress: Option<Arc<dyn Fn(BulkProgressEvent) + Send + Sync>>,
) -> Result<BulkRunReport, String> {
    config.validate()?;
    for &fmt in &config.preset.formats {
        let _ = super::planner::resolve_variant_coding(
            fmt,
            &config.preset.quality,
            config.preset.quality_fallback,
            config.preset.workspace_policy,
        )?;
    }

    let start = Instant::now();
    fs::create_dir_all(&config.output_dir).map_err(|e| {
        format!("failed creating output directory '{}': {e}", config.output_dir.display())
    })?;

    let discovered = discover_media_files(
        &config.inputs,
        Some(&config.output_dir),
        config.recursive,
        &config.include_globs,
        &config.exclude_globs,
        config.follow_symlinks,
    )?;
    let total = discovered.len();
    let recipe_hash = recipe_fingerprint(&config.preset, config.output_layout);
    let eng_fp = engine_fingerprint();

    let cache_path = config.output_dir.join(CACHE_FILE_NAME);
    let prior_cache: BulkCacheFile = if config.incremental_cache && !config.force_rebuild {
        fs::read_to_string(&cache_path)
            .ok()
            .and_then(|s| serde_json::from_str::<BulkCacheFile>(&s).ok())
            .filter(|c| c.schema == CACHE_SCHEMA && c.engine_fingerprint == eng_fp)
            .unwrap_or_default()
    } else {
        BulkCacheFile::default()
    };

    // Under FailFast with mutable layouts (Flatten / PreserveTree), defer overwriting prior assets
    // until ALL sources succeed so a late failure never leaves the old manifest pointing at new bytes!
    let defer_publish = config.on_error == FailurePolicy::FailFast
        && config.output_layout != OutputPathLayout::ContentAddressed;

    let stop_flag = cancel.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    let results: Mutex<Vec<(usize, Result<(ManifestAsset, bool, StagedDiskBatch), String>)>> =
        Mutex::new(Vec::with_capacity(total));
    let completed = AtomicU64::new(0);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.concurrency)
        .build()
        .map_err(|e| format!("failed creating worker pool: {e}"))?;

    pool.install(|| {
        use rayon::prelude::*;
        discovered.par_iter().enumerate().for_each(|(idx, item)| {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            let cached_entry = prior_cache.entries.get(&item.relative_key);
            let res = process_single_input(item, config, &recipe_hash, cached_entry, defer_publish);
            let done_n = completed.fetch_add(1, Ordering::Relaxed) as usize + 1;

            if let Some(ref cb) = on_progress {
                let ev = match &res {
                    Ok((asset, was_cached, _)) => {
                        let out_b: u64 = asset.variants.iter().map(|v| v.bytes).sum();
                        BulkProgressEvent {
                            index: done_n,
                            total,
                            relative_key: item.relative_key.clone(),
                            status: if *was_cached { "cached" } else { "encoded" }.to_string(),
                            variants_written: asset.variants.len(),
                            input_bytes: asset.source_bytes,
                            output_bytes: out_b,
                            error: None,
                        }
                    }
                    Err(err) => BulkProgressEvent {
                        index: done_n,
                        total,
                        relative_key: item.relative_key.clone(),
                        status: "failed".to_string(),
                        variants_written: 0,
                        input_bytes: item.size_bytes,
                        output_bytes: 0,
                        error: Some(err.clone()),
                    },
                };
                cb(ev);
            }

            if res.is_err() && config.on_error == FailurePolicy::FailFast {
                stop_flag.store(true, Ordering::Relaxed);
            }
            if let Ok(mut guard) = results.lock() {
                guard.push((idx, res));
            }
        });
    });

    let mut collected = results.into_inner().unwrap_or_default();
    collected.sort_by_key(|(idx, _)| *idx);

    let mut assets = Vec::new();
    let mut errors = Vec::new();
    let mut deferred_batch = StagedDiskBatch::new(config.output_dir.clone());
    let mut processed = 0usize;
    let mut skipped_cached = 0usize;
    let mut failed = 0usize;
    let mut new_cache = BulkCacheFile {
        schema: CACHE_SCHEMA.to_string(),
        engine_fingerprint: eng_fp,
        entries: prior_cache.entries,
    };

    for (_, outcome) in collected {
        match outcome {
            Ok((asset, was_cached, staged_disk)) => {
                if was_cached {
                    skipped_cached += 1;
                } else {
                    processed += 1;
                }
                deferred_batch.extend_from(staged_disk);
                new_cache.entries.insert(
                    asset.source_relative_path.clone(),
                    CachedAssetEntry {
                        source_sha256: asset.source_sha256.clone(),
                        recipe_sha256: recipe_hash.clone(),
                        asset: asset.clone(),
                    },
                );
                assets.push(asset);
            }
            Err(err) => {
                failed += 1;
                errors.push(err);
            }
        }
    }

    // Under FailFast, abort BEFORE publishing deferred assets (Drop cleans all staged temps!).
    if config.on_error == FailurePolicy::FailFast && failed > 0 {
        return Err(format!("bulk job aborted (fail_fast): {}", errors.join("; ")));
    }

    let total_input_bytes: u64 = assets.iter().map(|a| a.source_bytes).sum();
    let total_output_bytes: u64 = assets
        .iter()
        .flat_map(|a| a.variants.iter().chain(a.video_variants.iter()))
        .map(|v| v.bytes)
        .sum();

    let manifest = BulkManifest {
        schema: MANIFEST_SCHEMA_VERSION.to_string(),
        generated_at_utc: now_utc_iso(),
        generator: format!("imageflow_tool {}", imageflow_types::version::one_line_version()),
        preset: config.preset.clone(),
        total_input_bytes,
        total_output_bytes,
        assets,
    };

    let mut meta_files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    if config.incremental_cache {
        let cache_json = serde_json::to_vec_pretty(&new_cache)
            .map_err(|e| format!("cache serialize failed: {e}"))?;
        meta_files.push((cache_path, cache_json));
    }

    let manifest_path = if config.write_manifest {
        let p = config.output_dir.join("manifest.json");
        let json = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| format!("manifest serialize failed: {e}"))?;
        meta_files.push((p.clone(), json));
        Some(p)
    } else {
        None
    };

    let snippets_path = if config.write_snippets {
        let p = config.output_dir.join("snippets.html");
        let mut buf = String::from("<!-- Generated by imageflow_tool bulk -->\n");
        for asset in &manifest.assets {
            // Only emit alphanumeric asset IDs in HTML comments (never raw filenames).
            let safe_id: String = asset
                .id
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            let snippet = render_picture_snippet(
                asset,
                "",
                "(max-width: 768px) 100vw, 50vw",
                &safe_id,
                false,
            );
            buf.push_str(&format!("\n<!-- asset: {safe_id} -->\n{snippet}\n"));
        }
        meta_files.push((p.clone(), buf.into_bytes()));
        Some(p)
    } else {
        None
    };

    // Publish deferred mutable assets AND metadata (.imageflow-cache.json, manifest.json,
    // snippets.html) together in ONE unified atomic transaction so any late metadata error
    // restores all overwritten assets back to their prior generation too!
    if !meta_files.is_empty() {
        let meta_batch = stage_files_to_disk(&config.output_dir, &meta_files)?;
        deferred_batch.extend_from(meta_batch);
    }
    commit_staged_batch(&mut deferred_batch)?;

    Ok(BulkRunReport {
        dry_run: false,
        discovered: total,
        processed,
        skipped_cached,
        failed,
        total_input_bytes,
        total_output_bytes,
        elapsed_ms: start.elapsed().as_millis() as u64,
        manifest_path,
        snippets_path,
        errors,
        manifest,
    })
}

fn now_utc_iso() -> String {
    let dt = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

#[cfg(test)]
mod rollback_tests {
    use super::*;

    #[test]
    fn commit_staged_batch_restores_already_replaced_files_on_phase2_failure() {
        let root = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("tmp")
            .join(format!("if-r5-rollback-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let first = root.join("first.txt");
        let second = root.join("second.txt");
        let created = root.join("brand_new.txt");
        fs::write(&first, b"gen1-first").unwrap();
        fs::write(&second, b"gen1-second").unwrap();

        let mut batch = stage_files_to_disk(
            &root,
            &[
                (first.clone(), b"gen2-first".to_vec()),
                (created.clone(), b"gen2-new".to_vec()),
                (second.clone(), b"gen2-second".to_vec()),
            ],
        )
        .unwrap();

        // Delete ONLY the 3rd staged temp file AFTER staging so Phase 1 preflight + hardlink
        // backups succeed, Phase 2 ACTUALLY replaces `first.txt` and creates `brand_new.txt`,
        // and THEN fails mid-Phase 2 on `second.txt` (ENOENT), forcing real restore rollback!
        unlink_confined_entry(&root, &batch.entries[2].parent_rel, &batch.entries[2].tmp_name);

        let err = commit_staged_batch(&mut batch)
            .expect_err("missing 3rd temp file must trigger Phase 2 rollback");
        assert!(err.contains("failed atomic rename"), "unexpected error: {err}");
        assert_eq!(fs::read(&first).unwrap(), b"gen1-first", "first.txt must be restored to gen1");
        assert_eq!(fs::read(&second).unwrap(), b"gen1-second", "second.txt must remain gen1");
        assert!(!created.exists(), "newly created file must be unlinked during Phase 2 rollback");
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "zero temp/backup files may remain: {leftovers:?}");
        let _ = fs::remove_dir_all(&root);
    }
}
