# Plan: Per-Test Checksum Files — Using `zensim-regress`

## Context

The visual test infrastructure uses three monolithic JSON files (`checksums.json`, `alternate_checksums.json`, `actual.json`) for 128 tests. Problems:

- **Opaque git diffs**: hex string swaps in a 128-line JSON
- **47 alternate checksums suppress regression detection forever**
- **No forensic data**: no diffs or history to explain why a checksum changed
- **Checksums auto-update**: no explicit approval step

Replace with per-test TOML files using `zensim-regress`, which already provides the complete chain-of-trust system: TOML types, `ChecksumManager` workflow, architecture-aware tolerances, and forensic diff evidence.

## What `zensim-regress` provides

The `zensim-regress` crate (in the `zensim` workspace) implements everything the old plan called "Step 2: Create checksum_file.rs" and more:

| Need | `zensim-regress` provides |
|------|---------------------------|
| TOML types | `TestChecksumFile`, `ChecksumEntry`, `ChecksumDiff`, `ToleranceSpec`, `ImageInfo` |
| Read/write | `TestChecksumFile::read_from()`, `write_to()` |
| Name sanitization | `sanitize_name()`, `checksum_path()` |
| Active/authoritative queries | `active_checksums()`, `authoritative()`, `find_by_id()` |
| Hash computation | `ChecksumHasher` trait, `SeaHasher` (default) |
| Architecture detection | `detect_arch_tag()`, `arch_matches()`, per-arch tolerance overrides |
| Manager workflow | `ChecksumManager::check_pixels()`, `check_file()`, `check_hash()`, `accept()`, `reject()` |
| UPDATE/REPLACE modes | `UPDATE_CHECKSUMS=1`, `REPLACE_CHECKSUMS=1` env vars (built into ChecksumManager) |
| Reference images | `save_reference_image()`, auto-lookup for pixel comparison |
| Chain-of-trust diffs | `ChecksumDiff::from_report()` — auto-populated on accept |

### What imageflow still needs to provide (thin adapter)

- **Legacy hash format**: imageflow uses `"{hash1}_{hash2}"` checksums, not `"sea:{hex}"`. Either:
  - Implement `ChecksumHasher` for the legacy format, or
  - Pass legacy hashes as opaque strings to `check_hash()`
- **S3 integration**: `ChecksumManager` uses local reference images. Imageflow needs a wrapper that downloads from S3 when no local reference exists.
- **`InfoStats`**: `zensim-regress` has `ImageInfo` (width, height, format) but not imageflow's extended stats (avg_rgb, luma, r_range, alpha_opaque_pct). Add these as custom fields or extend `ImageInfo`.
- **`BitmapDiffStats` → `ChecksumDiff` bridge**: convert imageflow's existing diff stats into `ChecksumDiff` fields.

## File Format

Unchanged from original plan. Each test gets `tests/visuals/checksums/{sanitized_name}.toml`:

```toml
name = "cms_rec2020_pq"

[tolerance]
max_channel_delta = 2
min_score = 95.0

[[checksum]]
id = "0A19C26C9CBE6975F_0ACDBA442F8FEC212"
confidence = 10
commit = "dfc9aad2"
arch = ["x86_64-avx2"]
reason = "initial baseline"

[[checksum]]
id = "02B302D9F2AD6086C_0ACDBA442F8FEC212"
confidence = 10
commit = "dfc9aad2"
arch = ["aarch64"]
reason = "ARM NEON rounding"
[checksum.diff]
vs = "0A19C26C9CBE6975F_0ACDBA442F8FEC212"
zensim_score = 99.7
category = "RoundingError"
max_channel_delta = [2, 0, 0]
pixels_differing_pct = 0.3
rounding_bias_balanced = true

[[checksum]]
id = "0B7F23AAC812DE112_0ACDBA442F8FEC212"
confidence = 0
commit = "ac10b42e"
reason = "pre-CICP fix"
status = "wrong"
[checksum.diff]
vs = "0A19C26C9CBE6975F_0ACDBA442F8FEC212"
zensim_score = 12.3
category = "Unclassified"
max_channel_delta = [224, 198, 210]
pixels_differing_pct = 95.2

[info]
width = 32
height = 32
format = "BGRA"
```

### Changes from original plan

- **`[tolerance]` section** replaces per-entry `tolerance` strings. Structured fields (`max_channel_delta`, `min_score`, etc.) with per-arch overrides (`[tolerance.override.aarch64]`).
- **`arch` field** on entries — tracks which architectures produce each hash.
- **`[checksum.diff]`** uses zensim fields (`zensim_score`, `category`, `rounding_bias_balanced`) instead of dssim. Richer forensic data.
- **No per-entry `tolerance` string** — tolerance is per-test, not per-checksum.

### Core semantics (unchanged)

- **confidence > 0** → active; matching passes immediately (no S3 download)
- **confidence = 0** → inactive; kept for forensics and bisecting
- **Highest confidence** → authoritative; S3 reference for pixel comparison on mismatch
- **`[checksum.diff]`** → chain of trust evidence
- **`status`** → informational label
- **`reason`** → why this checksum was added or retired

## Update Workflow

Same as original, now handled by `ChecksumManager`:

**Normal run** (`cargo test`): `ChecksumManager::check_hash()` or `check_pixels()`. Match → pass. No match → download authoritative reference from S3, compare, pass/fail.

**Explicit update** (`UPDATE_CHECKSUMS=1 cargo test`): `ChecksumManager` auto-accepts within-tolerance mismatches with chain-of-trust diff evidence. Only appends.

**Replace** (`REPLACE_CHECKSUMS=1 cargo test`): `ChecksumManager` retires all active entries, adds new baseline.

## Implementation Steps

### Step 1: Add `zensim-regress` dependency

**File**: `imageflow_core/Cargo.toml`

```toml
[dev-dependencies]
zensim-regress = { path = "../zensim/zensim-regress" }
# or via git:
# zensim-regress = { git = "...", branch = "main" }
```

No need for separate `toml` or `serde` dev-deps — `zensim-regress` handles serialization internally.

### Step 2: Create adapter layer

**New file**: `imageflow_core/tests/common/checksum_adapter.rs`

Thin adapter between imageflow's test infrastructure and `zensim-regress`:

```rust
use zensim_regress::manager::{ChecksumManager, CheckResult};
use zensim_regress::checksum_file::TestChecksumFile;

/// Wrap ChecksumManager with S3 reference image download.
pub struct ImageflowCheckManager {
    inner: ChecksumManager,
    s3_ctx: S3Context,  // existing S3 upload/download
}

impl ImageflowCheckManager {
    pub fn new(checksum_dir: &Path, s3: S3Context) -> Self { ... }

    /// Check using legacy hash format.
    /// Calls inner.check_hash() then handles S3 download for comparison.
    pub fn check(&self, test_name: &str, legacy_hash: &str,
                 actual_bitmap: &BitmapWindowMut<u8>) -> CheckResult { ... }
}

/// Convert BitmapDiffStats to ChecksumDiff fields.
fn bitmap_diff_to_checksum_diff(stats: &BitmapDiffStats, vs: &str) -> ChecksumDiff { ... }
```

### Step 3: Modify `exact_match()` in `common/mod.rs`

**File**: `imageflow_core/tests/common/mod.rs`

Replace the `checksums.json` / `alternate_checksums.json` lookup with:

1. Try `ChecksumManager::check_hash(test_name, actual_hash)`
2. `CheckResult::Match` → return `ChecksumMatch::Match` with authoritative ID for S3
3. `CheckResult::NoBaseline` → fall back to existing JSON logic (dual-read during migration)
4. `CheckResult::Failed` → `ChecksumMatch::Mismatch` with authoritative ID
5. `CheckResult::WithinTolerance` → pass (with auto-accept if UPDATE mode)

### Step 4: Modify `evaluate_result()` for comparison path

**File**: `imageflow_core/tests/common/mod.rs`

On mismatch, the existing code downloads from S3 and compares bitmaps. Replace the tolerance/diff logic:

- Convert `BitmapDiffStats` to `ChecksumDiff` via adapter
- Use `ChecksumManager::accept()` with the diff in UPDATE mode
- Use `ChecksumManager::reject()` equivalent for REPLACE mode (retire old entries)

### Step 5: Migration

A `#[test]` gated behind `MIGRATE_CHECKSUMS=1`:

1. Read `checksums.json` → 128 entries (name → primary hash)
2. Read `alternate_checksums.json` → 47 entries (primary → [alternate hashes])
3. For each test, create `TestChecksumFile` using `zensim-regress` types:
   - Primary → `ChecksumEntry { id, confidence: 10, commit: "migrated", reason: "migrated from JSON" }`
   - Each alternate → `ChecksumEntry { id, confidence: 10, commit: "migrated", reason: "migrated alternate" }` with no diff
4. Write via `TestChecksumFile::write_to()`

Diffs are `None` after migration — populated on next `UPDATE_CHECKSUMS=1` run when S3 references are available for comparison.

### Step 6: Verify

1. `MIGRATE_CHECKSUMS=1 cargo test --test visuals migrate_checksums`
2. `cargo test --test visuals` — all 93 tests pass (TOML first, JSON fallback)
3. Commit 128 TOML files
4. Optionally `UPDATE_CHECKSUMS=1 cargo test --test visuals` to populate diffs and info

### Step 7: Cleanup (separate commit)

- Remove `checksums.json`, `alternate_checksums.json`, `actual.json`
- Remove `LazyLock<RwLock<BTreeMap>>` statics and legacy JSON methods
- Simplify `ChecksumCtx` to delegate to `ImageflowCheckManager`

## Files Modified

| File | Change |
|------|--------|
| `imageflow_core/Cargo.toml` | Add `zensim-regress` dev-dep |
| `imageflow_core/tests/common/mod.rs` | Add module, modify `exact_match()`, `evaluate_result()` |
| `imageflow_core/tests/common/checksum_adapter.rs` | **NEW** — thin adapter (S3 + legacy hash bridge) |
| `imageflow_core/tests/visuals/checksums/*.toml` | **NEW** — 128 generated files |

## Files NOT Modified

| File | Reason |
|------|--------|
| `imageflow_core/tests/visuals.rs` | All 93 tests unchanged |
| `imageflow_core/tests/common/bitmap_diff_stats.rs` | Unchanged; provides data for adapter |
| Upload infrastructure | Unchanged |

## Key Differences from Original Plan

1. **No `checksum_file.rs` in imageflow** — all types come from `zensim-regress`
2. **No reimplemented TOML types** — `TestChecksumFile`, `ChecksumEntry`, `ChecksumDiff` etc. are battle-tested with 100 tests
3. **`ChecksumManager` handles UPDATE/REPLACE/Normal modes** — no manual env var checking
4. **Architecture-aware tolerances** with per-arch overrides built in
5. **Zensim-based diffs** replace dssim — richer classification (RoundingError, ChannelSwap, etc.)
6. **Adapter layer is ~100 lines** instead of ~400 lines of reimplemented infrastructure

## Verification

1. `cargo test --test visuals` — all 93 pass (dual-read)
2. `UPDATE_CHECKSUMS=1 cargo test --test visuals` — diffs populated
3. Inspect TOML files — confidence ratings correct, alternates have confidence=10
4. `git diff -- tests/visuals/checksums/` — diffs readable and self-documenting
5. Remove JSON files → tests still pass from TOML files alone
6. Push → CI passes all platforms
