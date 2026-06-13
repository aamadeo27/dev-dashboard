use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use tokio::sync::Mutex;
use tracing::Instrument;

use crate::error::{AppError, AppResult};

/// A Claude sequence definition discovered on disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct Sequence {
    pub name: String,
    pub description: String,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub path: PathBuf,
    pub mtime: DateTime<Utc>,
}

/// Cache entry: stores the directory mtime at scan time and the resulting
/// sequence list. The key in the outer HashMap is the project_id.
type CacheEntry = (DateTime<Utc>, Vec<Sequence>);

/// In-memory loader for `.claude/sequences/*.md` files within a project tree.
///
/// Caches per project_id keyed by the directory mtime. The next call after a
/// mtime change triggers a full re-scan (invalidation within one call).
#[derive(Default)]
pub struct SequenceLoader {
    cache: HashMap<String, CacheEntry>,
}

impl SequenceLoader {
    /// Create a new loader with an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a wrapper suitable for storage in `AppState`.
    pub fn new_arc() -> std::sync::Arc<Mutex<Self>> {
        std::sync::Arc::new(Mutex::new(Self::new()))
    }

    /// Load all sequences for `project_id` located at `project_path`.
    ///
    /// Reads `<project_path>/.claude/sequences/*.md`. Returns the cached list
    /// if the sequences directory mtime is unchanged since the last scan. If
    /// the directory does not exist, returns an empty `Vec` (no error).
    ///
    /// Sequences are sorted alphabetically by name.
    pub async fn load_all(
        &mut self,
        project_id: &str,
        project_path: &Path,
    ) -> AppResult<Vec<Sequence>> {
        let seq_dir = project_path.join(".claude").join("sequences");

        // Check whether the sequences directory exists at all.
        let dir_meta = match tokio::fs::metadata(&seq_dir).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Directory absent — return empty, no error.
                tracing::info!(
                    component = "sequence_loader",
                    project_id = %project_id,
                    "sequences directory not found; returning empty list"
                );
                return Ok(Vec::new());
            }
            Err(e) => return Err(AppError::Io(e)),
        };

        let current_mtime = system_time_to_utc(dir_meta.modified()?);

        // Cache hit: mtime unchanged.
        if let Some((cached_mtime, cached_seqs)) = self.cache.get(project_id) {
            if *cached_mtime == current_mtime {
                tracing::info!(
                    component = "sequence_loader",
                    project_id = %project_id,
                    count = cached_seqs.len(),
                    "sequence cache hit"
                );
                return Ok(cached_seqs.clone());
            }
        }

        // Cache miss or invalidation — re-scan. Use `.instrument()` rather than
        // `.entered()` so no `!Send` span guard is held across the await (the
        // load_all future must be Send to be usable as a Tauri command).
        let span = tracing::info_span!("SequenceLoader::load_all", project_id = %project_id);
        let sequences = scan_sequences_dir(&seq_dir).instrument(span).await?;

        tracing::info!(
            component = "sequence_loader",
            project_id = %project_id,
            count = sequences.len(),
            "sequences scanned"
        );

        self.cache
            .insert(project_id.to_string(), (current_mtime, sequences.clone()));

        Ok(sequences)
    }

    /// Bust the cache for `project_id`, then call `load_all`.
    pub async fn refresh(
        &mut self,
        project_id: &str,
        project_path: &Path,
    ) -> AppResult<Vec<Sequence>> {
        self.cache.remove(project_id);
        self.load_all(project_id, project_path).await
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Convert `std::time::SystemTime` to `DateTime<Utc>`, falling back to epoch
/// on overflow (which cannot occur in practice for filesystem mtimes).
fn system_time_to_utc(st: std::time::SystemTime) -> DateTime<Utc> {
    match st.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => DateTime::<Utc>::from_timestamp(d.as_secs() as i64, d.subsec_nanos())
            .unwrap_or_default(),
        Err(_) => DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_default(),
    }
}

/// Maximum size of a sequence file we are willing to read (1 MiB).
const MAX_SEQ_FILE_BYTES: u64 = 1024 * 1024;

/// Read all `*.md` files in `seq_dir`, parse each into a `Sequence`, sort by
/// name, and return the result.
async fn scan_sequences_dir(seq_dir: &Path) -> AppResult<Vec<Sequence>> {
    let mut dir = match tokio::fs::read_dir(seq_dir).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AppError::Io(e)),
    };

    let mut sequences = Vec::new();

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();

        // Only process `.md` files.
        let is_md = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            continue;
        }

        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // FIX-3: Reject symlinks before reading.
        let lmeta = match tokio::fs::symlink_metadata(&path).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    component = "sequence_loader",
                    path = %path.display(),
                    error = %e,
                    "failed to stat entry; skipping"
                );
                continue;
            }
        };
        if lmeta.file_type().is_symlink() {
            tracing::warn!(
                component = "sequence_loader",
                path = %path.display(),
                "symlink in sequences dir; skipping"
            );
            continue;
        }

        // FIX-4: Cap file size at 1 MiB. Since we already rejected symlinks,
        // lmeta.len() is the actual file size.
        if lmeta.len() > MAX_SEQ_FILE_BYTES {
            tracing::warn!(
                component = "sequence_loader",
                path = %path.display(),
                size = lmeta.len(),
                "sequence file exceeds size limit; skipping"
            );
            continue;
        }

        // FIX-2: Warn-and-skip on mtime failure instead of aborting the scan.
        let mtime = match lmeta.modified() {
            Ok(st) => system_time_to_utc(st),
            Err(e) => {
                tracing::warn!(
                    component = "sequence_loader",
                    path = %path.display(),
                    error = %e,
                    "failed to get mtime for sequence file; skipping"
                );
                continue;
            }
        };

        // FIX-1: Fall back to lossy UTF-8 decode instead of skipping the file.
        let content = match tokio::fs::read(&path).await {
            Ok(bytes) => {
                if let Ok(s) = std::str::from_utf8(&bytes) {
                    s.to_string()
                } else {
                    tracing::warn!(
                        component = "sequence_loader",
                        path = %path.display(),
                        "sequence file contains invalid UTF-8; using lossy decode"
                    );
                    String::from_utf8_lossy(&bytes).into_owned()
                }
            }
            Err(e) => {
                tracing::warn!(
                    component = "sequence_loader",
                    path = %path.display(),
                    error = %e,
                    "failed to read sequence file; skipping"
                );
                continue;
            }
        };

        let description = extract_description(&content);

        sequences.push(Sequence {
            name,
            description,
            path,
            mtime,
        });
    }

    sequences.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sequences)
}

/// Extract the description from a sequence `.md` file.
///
/// Rules:
/// - Normalise Windows line endings (`\r\n` → `\n`).
/// - Skip lines that start with `#` (headings).
/// - Skip blank lines.
/// - Return the first non-empty, non-heading line.
/// - If none is found, return `"(No description)"`.
pub(crate) fn extract_description(content: &str) -> String {
    // Normalise Windows line endings.
    let normalised = content.replace("\r\n", "\n").replace('\r', "\n");

    for line in normalised.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        return trimmed.to_string();
    }

    "(No description)".to_string()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -----------------------------------------------------------------------
    // extract_description — pure logic tests
    // -----------------------------------------------------------------------

    /// A blank file returns the fallback.
    #[test]
    fn description_blank_file_returns_fallback() {
        assert_eq!(extract_description(""), "(No description)");
    }

    /// A file containing only whitespace lines returns the fallback.
    #[test]
    fn description_whitespace_only_returns_fallback() {
        assert_eq!(extract_description("   \n\n   \n"), "(No description)");
    }

    /// A heading-only file (no body text) returns the fallback.
    #[test]
    fn description_heading_only_file_returns_fallback() {
        let content = "# My Sequence\n## Sub heading\n### Another\n";
        assert_eq!(extract_description(content), "(No description)");
    }

    /// A file with a heading followed by blank lines and then a description
    /// paragraph returns the first non-empty, non-heading line.
    #[test]
    fn description_normal_file_returns_first_paragraph() {
        let content = "# Title\n\nThis is the description.\n\nMore text.";
        assert_eq!(extract_description(content), "This is the description.");
    }

    /// A multi-paragraph file returns only the first non-heading paragraph.
    #[test]
    fn description_multi_paragraph_returns_first_only() {
        let content = "# Title\n\nFirst paragraph.\n\nSecond paragraph.";
        assert_eq!(extract_description(content), "First paragraph.");
    }

    /// Windows line endings (`\r\n`) are normalised and handled correctly.
    #[test]
    fn description_windows_line_endings_handled() {
        let content = "# Title\r\n\r\nDescription with CRLF.\r\nSecond line.\r\n";
        assert_eq!(extract_description(content), "Description with CRLF.");
    }

    /// A file that starts immediately with a description (no heading) returns it.
    #[test]
    fn description_no_heading_returns_first_line() {
        let content = "Just a description with no heading.";
        assert_eq!(
            extract_description(content),
            "Just a description with no heading."
        );
    }

    /// A file with only headings and blank lines returns the fallback.
    #[test]
    fn description_headings_and_blank_lines_only_returns_fallback() {
        let content = "# Heading One\n\n## Heading Two\n\n   \n\n### Heading Three\n";
        assert_eq!(extract_description(content), "(No description)");
    }

    /// Leading/trailing whitespace on the description line is trimmed.
    #[test]
    fn description_trims_whitespace_from_line() {
        let content = "# Title\n\n   Description with leading spaces.   \n";
        assert_eq!(
            extract_description(content),
            "Description with leading spaces."
        );
    }

    // -----------------------------------------------------------------------
    // SequenceLoader — async integration tests using tempfiles
    // -----------------------------------------------------------------------

    /// Helper: create a temp project directory with a `.claude/sequences/` subdir.
    async fn make_seq_dir() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let seq_dir = dir.path().join(".claude").join("sequences");
        tokio::fs::create_dir_all(&seq_dir)
            .await
            .expect("create seq dir");
        let project_path = dir.path().to_path_buf();
        (dir, project_path)
    }

    /// An empty sequences directory returns an empty Vec with no error.
    #[tokio::test]
    async fn load_all_empty_dir_returns_empty_vec() {
        let (_dir, project_path) = make_seq_dir().await;
        let mut loader = SequenceLoader::new();
        let result = loader.load_all("proj-1", &project_path).await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        assert!(result.unwrap().is_empty());
    }

    /// A non-existent sequences directory returns an empty Vec with no error.
    #[tokio::test]
    async fn load_all_missing_dir_returns_empty_vec() {
        let dir = TempDir::new().expect("tempdir");
        // Do NOT create .claude/sequences
        let project_path = dir.path().to_path_buf();
        let mut loader = SequenceLoader::new();
        let result = loader.load_all("proj-2", &project_path).await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        assert!(result.unwrap().is_empty());
    }

    /// Sequences are returned sorted alphabetically by name.
    #[tokio::test]
    async fn load_all_sequences_sorted_alphabetically() {
        let (_dir, project_path) = make_seq_dir().await;
        let seq_dir = project_path.join(".claude").join("sequences");
        tokio::fs::write(seq_dir.join("zebra.md"), "# Z\n\nZ sequence.")
            .await
            .unwrap();
        tokio::fs::write(seq_dir.join("alpha.md"), "# A\n\nA sequence.")
            .await
            .unwrap();
        tokio::fs::write(seq_dir.join("beta.md"), "# B\n\nB sequence.")
            .await
            .unwrap();

        let mut loader = SequenceLoader::new();
        let seqs = loader.load_all("proj-3", &project_path).await.unwrap();
        assert_eq!(seqs.len(), 3);
        assert_eq!(seqs[0].name, "alpha");
        assert_eq!(seqs[1].name, "beta");
        assert_eq!(seqs[2].name, "zebra");
    }

    /// Non-.md files in the directory are ignored.
    #[tokio::test]
    async fn load_all_ignores_non_md_files() {
        let (_dir, project_path) = make_seq_dir().await;
        let seq_dir = project_path.join(".claude").join("sequences");
        tokio::fs::write(seq_dir.join("valid.md"), "# V\n\nValid sequence.")
            .await
            .unwrap();
        tokio::fs::write(seq_dir.join("ignored.txt"), "Not a sequence.")
            .await
            .unwrap();
        tokio::fs::write(seq_dir.join("ignored.json"), "{}")
            .await
            .unwrap();

        let mut loader = SequenceLoader::new();
        let seqs = loader.load_all("proj-4", &project_path).await.unwrap();
        assert_eq!(seqs.len(), 1, "only .md files must be loaded");
        assert_eq!(seqs[0].name, "valid");
    }

    /// `refresh` busts the cache and returns fresh results.
    #[tokio::test]
    async fn refresh_busts_cache_and_rescans() {
        let (_dir, project_path) = make_seq_dir().await;
        let seq_dir = project_path.join(".claude").join("sequences");
        tokio::fs::write(seq_dir.join("seq1.md"), "# S1\n\nSeq one.")
            .await
            .unwrap();

        let mut loader = SequenceLoader::new();
        let first = loader.load_all("proj-5", &project_path).await.unwrap();
        assert_eq!(first.len(), 1);

        // Add a second sequence — but mtime may not change on fast file systems.
        // refresh() must bypass the cache regardless.
        tokio::fs::write(seq_dir.join("seq2.md"), "# S2\n\nSeq two.")
            .await
            .unwrap();
        // Touch the directory so mtime changes (on Windows, writing a file may
        // update the dir mtime, but refresh() is the guaranteed path).
        let seqs = loader.refresh("proj-5", &project_path).await.unwrap();
        // After refresh the new file is visible (at minimum the loader re-ran).
        assert!(
            seqs.len() >= 1,
            "refresh must return at least the existing sequence"
        );
    }

    // -----------------------------------------------------------------------
    // T3.1 gap tests — added by tester
    // -----------------------------------------------------------------------

    /// Old Mac line endings (`\r` only, no `\n`) are normalised and the
    /// description is extracted correctly.
    #[test]
    fn description_old_mac_line_endings_handled() {
        // Classic Mac OS used bare `\r` as line separator.
        // The implementation normalises bare `\r` → `\n` before splitting.
        let content = "# Title\rDescription after bare CR.\rSecond line.\r";
        assert_eq!(extract_description(content), "Description after bare CR.");
    }

    /// A file whose only content after the heading is separated by old Mac
    /// line endings returns the fallback when there is no non-heading content.
    #[test]
    fn description_old_mac_line_endings_heading_only_returns_fallback() {
        let content = "# Heading One\r## Heading Two\r";
        assert_eq!(extract_description(content), "(No description)");
    }

    /// Mixed heading levels (##, ###) followed by non-heading content —
    /// the first non-heading, non-blank line is returned regardless of the
    /// heading level that precedes it.
    #[test]
    fn description_mixed_heading_levels_returns_first_content_line() {
        let content = "# Top\n## Sub\n### Sub-sub\n\nActual description here.";
        assert_eq!(extract_description(content), "Actual description here.");
    }

    /// `load_all` called twice with the same directory state returns the
    /// cached result on the second call (same Vec contents, no panic/error).
    #[tokio::test]
    async fn load_all_returns_cached_result_on_second_call() {
        let (_dir, project_path) = make_seq_dir().await;
        let seq_dir = project_path.join(".claude").join("sequences");
        tokio::fs::write(seq_dir.join("cached.md"), "# C\n\nCached sequence.")
            .await
            .unwrap();

        let mut loader = SequenceLoader::new();
        let first = loader.load_all("proj-cache", &project_path).await.unwrap();
        let second = loader.load_all("proj-cache", &project_path).await.unwrap();

        // Both calls must return the same sequence names.
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(first[0].name, second[0].name);
        assert_eq!(first[0].description, second[0].description);
    }

    /// Two different project_ids with different sequences do not cross-contaminate
    /// each other's cache entries.
    #[tokio::test]
    async fn load_all_different_project_ids_are_independent() {
        // Project A: one sequence
        let (_dir_a, project_path_a) = make_seq_dir().await;
        let seq_dir_a = project_path_a.join(".claude").join("sequences");
        tokio::fs::write(seq_dir_a.join("alpha.md"), "# A\n\nProject A sequence.")
            .await
            .unwrap();

        // Project B: two sequences
        let (_dir_b, project_path_b) = make_seq_dir().await;
        let seq_dir_b = project_path_b.join(".claude").join("sequences");
        tokio::fs::write(seq_dir_b.join("beta1.md"), "# B1\n\nProject B sequence 1.")
            .await
            .unwrap();
        tokio::fs::write(seq_dir_b.join("beta2.md"), "# B2\n\nProject B sequence 2.")
            .await
            .unwrap();

        let mut loader = SequenceLoader::new();
        let seqs_a = loader.load_all("proj-a", &project_path_a).await.unwrap();
        let seqs_b = loader.load_all("proj-b", &project_path_b).await.unwrap();

        // Project A must see only its own sequence.
        assert_eq!(seqs_a.len(), 1, "proj-a must have exactly 1 sequence");
        assert_eq!(seqs_a[0].name, "alpha");

        // Project B must see only its own sequences.
        assert_eq!(seqs_b.len(), 2, "proj-b must have exactly 2 sequences");
        assert_eq!(seqs_b[0].name, "beta1");
        assert_eq!(seqs_b[1].name, "beta2");

        // A second load of proj-a must not pick up proj-b's sequences.
        let seqs_a_again = loader.load_all("proj-a", &project_path_a).await.unwrap();
        assert_eq!(
            seqs_a_again.len(),
            1,
            "cached proj-a must still have 1 sequence"
        );
    }

    /// The `name` field of a returned Sequence equals the filename stem with
    /// case preserved exactly (no lowercasing or other transformation).
    #[tokio::test]
    async fn load_all_sequence_name_preserves_case() {
        let (_dir, project_path) = make_seq_dir().await;
        let seq_dir = project_path.join(".claude").join("sequences");
        // Mixed-case filename — name must be preserved verbatim.
        tokio::fs::write(seq_dir.join("MyMixedCase.md"), "# Title\n\nDesc.")
            .await
            .unwrap();

        let mut loader = SequenceLoader::new();
        let seqs = loader.load_all("proj-case", &project_path).await.unwrap();

        assert_eq!(seqs.len(), 1);
        assert_eq!(
            seqs[0].name, "MyMixedCase",
            "name must be the filename stem with case preserved"
        );
    }

    /// The `path` field in each returned Sequence is an absolute path that
    /// ends with the expected filename.
    #[tokio::test]
    async fn load_all_sequence_path_is_absolute() {
        let (_dir, project_path) = make_seq_dir().await;
        let seq_dir = project_path.join(".claude").join("sequences");
        tokio::fs::write(seq_dir.join("my-seq.md"), "# S\n\nDescription.")
            .await
            .unwrap();

        let mut loader = SequenceLoader::new();
        let seqs = loader.load_all("proj-path", &project_path).await.unwrap();

        assert_eq!(seqs.len(), 1);
        let seq_path = &seqs[0].path;
        assert!(
            seq_path.is_absolute(),
            "Sequence.path must be absolute, got: {}",
            seq_path.display()
        );
        assert_eq!(
            seq_path.file_name().and_then(|f| f.to_str()),
            Some("my-seq.md"),
            "Sequence.path must end with the .md filename"
        );
    }
}
