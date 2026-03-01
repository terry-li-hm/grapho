use crate::models::Section;
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryDoc {
    pub header_lines: Vec<String>,
    pub sections: Vec<Section>,
    pub trailing_newline: bool,
}

pub fn read_doc(path: &Path) -> Result<MemoryDoc> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    parse_doc(&content)
}

pub fn parse_doc(content: &str) -> Result<MemoryDoc> {
    let trailing_newline = content.ends_with('\n');
    let mut header_lines = Vec::new();
    let mut sections = Vec::new();
    let mut current: Option<Section> = None;
    let mut seen_section = false;

    for raw_line in content.split('\n') {
        let line = raw_line.trim_end_matches('\r');

        if let Some(name) = parse_section_header(line) {
            seen_section = true;
            if let Some(section) = current.take() {
                sections.push(section);
            }
            current = Some(Section {
                name,
                entries: Vec::new(),
            });
            continue;
        }

        if !seen_section {
            header_lines.push(line.to_string());
            continue;
        }

        if line.is_empty() {
            continue;
        }

        if let Some(section) = current.as_mut() {
            section.entries.push(line.to_string());
        } else {
            return Err(anyhow!("parser state invalid: content exists without a section"));
        }
    }

    if let Some(section) = current {
        sections.push(section);
    }

    Ok(MemoryDoc {
        header_lines,
        sections,
        trailing_newline,
    })
}

pub fn render_doc(doc: &MemoryDoc) -> String {
    let mut lines = Vec::new();

    lines.extend(doc.header_lines.iter().cloned());

    if !doc.header_lines.is_empty() && !doc.sections.is_empty() {
        while matches!(lines.last(), Some(last) if !last.is_empty()) {
            lines.push(String::new());
        }
    }

    for (index, section) in doc.sections.iter().enumerate() {
        lines.push(format!("## {}", section.name));
        for entry in &section.entries {
            lines.push(entry.clone());
        }
        if index + 1 < doc.sections.len() {
            lines.push(String::new());
        }
    }

    let mut content = lines.join("\n");
    if doc.trailing_newline {
        content.push('\n');
    }
    content
}

pub fn write_doc_atomic(path: &Path, doc: &MemoryDoc) -> Result<()> {
    let content = render_doc(doc);
    write_string_atomic(path, &content)
}

pub fn write_string_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent directory: {}", parent.display()))?;
    }

    let tmp_path = tmp_path(path);
    fs::write(&tmp_path, content)
        .with_context(|| format!("failed to write temp file {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to rename temp file {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.tmp", path.display()))
}

fn parse_section_header(line: &str) -> Option<String> {
    line.strip_prefix("## ").and_then(|rest| {
        let name = rest.trim();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_doc, read_doc, render_doc};
    use crate::paths::resolve_memory_path;

    #[test]
    fn round_trip_actual_memory_file_is_identical() {
        let path = resolve_memory_path().expect("memory path should resolve");
        assert!(
            path.exists(),
            "expected MEMORY.md to exist at {}",
            path.display()
        );

        let original = std::fs::read_to_string(&path).expect("should read MEMORY.md");
        let parsed = read_doc(&path).expect("should parse MEMORY.md");
        let rendered = render_doc(&parsed);

        assert_eq!(rendered, original, "round-trip must preserve file identity");
        let reparsed = parse_doc(&rendered).expect("re-parse should succeed");
        assert_eq!(parsed, reparsed, "parse -> render -> parse should be stable");
    }
}
