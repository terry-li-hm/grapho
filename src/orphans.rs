use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A cache finding file with no authoritative counterpart, before age filtering.
pub struct Candidate {
    pub name: String,
    pub path: PathBuf,
}

/// Normalize a file stem for basename matching: lowercase, unify '-' and '_'.
fn normalize_stem(stem: &str) -> String {
    stem.to_ascii_lowercase().replace('_', "-")
}

/// Strip a leading `---\n...\n---\n` YAML frontmatter block and trim, for content equality.
fn body_only(content: &str) -> String {
    let body = if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(idx) = rest.find("\n---\n") {
            &rest[idx + "\n---\n".len()..]
        } else if let Some(idx) = rest.find("\n---") {
            &rest[idx + "\n---".len()..]
        } else {
            rest
        }
    } else {
        content
    };
    body.trim().to_string()
}

/// True if this finding file is a promotion stub pointing elsewhere (already handled).
fn is_promotion_stub(body: &str) -> bool {
    let b = body.trim_start();
    b.starts_with("Promoted ")
        && (b.contains("epigenome/marks")
            || b.contains("epistemics")
            || b.contains("authoritative mark"))
}

/// True if content's YAML frontmatter declares `node_type: memory`.
fn has_memory_node_type(content: &str) -> bool {
    let Some(rest) = content.strip_prefix("---\n") else {
        return false;
    };
    let fm_end = rest.find("\n---").unwrap_or(rest.len());
    let fm = &rest[..fm_end];
    fm.lines().any(|line| line.trim() == "node_type: memory")
}

/// True if a memory finding file is a CC-auto-memory finding worth checking
/// (filename starts with "finding" OR frontmatter has `node_type: memory`),
/// and is not the MEMORY.md / memory-overflow.md index files.
fn is_candidate_file(stem: &str, content: &str) -> bool {
    let lower = stem.to_ascii_lowercase();
    if lower == "memory" || lower == "memory-overflow" {
        return false;
    }
    lower.starts_with("finding") || has_memory_node_type(content)
}

/// Pure detector: cache findings with no marks counterpart (by basename OR body), excluding stubs.
pub fn find_orphan_candidates(memory_dir: &Path, marks_dir: &Path) -> Result<Vec<Candidate>> {
    let mut mark_stems: HashSet<String> = HashSet::new();
    let mut mark_bodies: HashSet<String> = HashSet::new();

    if marks_dir.exists() {
        for entry in fs::read_dir(marks_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            mark_stems.insert(normalize_stem(stem));
            if let Ok(content) = fs::read_to_string(&path) {
                mark_bodies.insert(body_only(&content));
            }
        }
    }

    let mut out = Vec::new();
    if !memory_dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(memory_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let content = fs::read_to_string(&path)?;
        if !is_candidate_file(&stem, &content) {
            continue;
        }
        let body = body_only(&content);
        if is_promotion_stub(&body) {
            continue;
        }
        if mark_stems.contains(&normalize_stem(&stem)) {
            continue;
        }
        if mark_bodies.contains(&body) {
            continue;
        }
        out.push(Candidate { name: stem, path });
    }
    Ok(out)
}

/// Pure age test (no fs): days between mtime and now, both unix seconds.
pub fn older_than(mtime_unix: i64, now_unix: i64, min_age_days: i64) -> bool {
    (now_unix - mtime_unix) / 86_400 >= min_age_days
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write_fixture(dir: &Path, stem: &str, body: &str) -> PathBuf {
        let path = dir.join(format!("{stem}.md"));
        let content = format!("---\nname: {stem}\nmetadata:\n  node_type: memory\n---\n\n{body}\n");
        fs::write(&path, content).unwrap();
        path
    }

    fn write_plain(dir: &Path, stem: &str, body: &str) -> PathBuf {
        let path = dir.join(format!("{stem}.md"));
        let content = format!("---\nname: {stem}\n---\n\n{body}\n");
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn orphan_when_no_counterpart() {
        let mem = tempfile::tempdir().unwrap();
        let marks = tempfile::tempdir().unwrap();
        write_fixture(mem.path(), "finding-foo", "ordinary body");
        let cands = find_orphan_candidates(mem.path(), marks.path()).unwrap();
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].name, "finding-foo");
    }

    #[test]
    fn not_orphan_when_basename_counterpart() {
        let mem = tempfile::tempdir().unwrap();
        let marks = tempfile::tempdir().unwrap();
        write_fixture(mem.path(), "finding-foo", "body one");
        write_plain(marks.path(), "finding-foo", "different body");
        let cands = find_orphan_candidates(mem.path(), marks.path()).unwrap();
        assert_eq!(cands.len(), 0);
    }

    #[test]
    fn basename_dash_underscore_variant() {
        let mem = tempfile::tempdir().unwrap();
        let marks = tempfile::tempdir().unwrap();
        write_fixture(mem.path(), "finding-foo-bar", "body one");
        write_plain(marks.path(), "finding_foo_bar", "different body");
        let cands = find_orphan_candidates(mem.path(), marks.path()).unwrap();
        assert_eq!(cands.len(), 0);
    }

    #[test]
    fn not_orphan_when_content_matches() {
        let mem = tempfile::tempdir().unwrap();
        let marks = tempfile::tempdir().unwrap();
        write_fixture(mem.path(), "finding-a", "shared body text");
        write_plain(marks.path(), "finding-renamed", "shared body text");
        let cands = find_orphan_candidates(mem.path(), marks.path()).unwrap();
        assert_eq!(cands.len(), 0);
    }

    #[test]
    fn stub_is_not_an_orphan() {
        let mem = tempfile::tempdir().unwrap();
        let marks = tempfile::tempdir().unwrap();
        let stub_body = "Promoted 2026-06-20 to the authoritative mark `~/epigenome/marks/finding-baz.md` (see chromatin).";
        write_fixture(mem.path(), "finding-baz", stub_body);
        let cands = find_orphan_candidates(mem.path(), marks.path()).unwrap();
        assert_eq!(cands.len(), 0);
    }

    #[test]
    fn skips_index_files() {
        let mem = tempfile::tempdir().unwrap();
        let marks = tempfile::tempdir().unwrap();
        fs::write(
            mem.path().join("MEMORY.md"),
            "# MEMORY\n\n## Section\n\n- entry\n",
        )
        .unwrap();
        fs::write(
            mem.path().join("memory-overflow.md"),
            "# MEMORY Overflow\n\n## Section\n\n- entry\n",
        )
        .unwrap();
        write_fixture(mem.path(), "finding-x", "ordinary body");
        let cands = find_orphan_candidates(mem.path(), marks.path()).unwrap();
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].name, "finding-x");
    }

    #[test]
    fn older_than_is_pure() {
        assert!(older_than(0, 8 * 86_400, 7));
        assert!(!older_than(0, 6 * 86_400, 7));
        assert!(older_than(0, 7 * 86_400, 7));
    }
}
