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
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_doc(&content)
}

pub fn parse_doc(content: &str) -> Result<MemoryDoc> {
    let trailing_newline = content.ends_with('\n');
    // Drop the single trailing newline so split('\n') doesn't yield a phantom
    // empty line; genuine trailing blank lines then survive as data.
    let body = content.strip_suffix('\n').unwrap_or(content);

    let mut header_lines = Vec::new();
    let mut sections = Vec::new();
    let mut current: Option<Section> = None;
    let mut seen_section = false;
    let mut seen_entry_in_current = false;

    for raw_line in body.split('\n') {
        let line = raw_line.trim_end_matches('\r');

        if let Some(name) = parse_section_header(line) {
            seen_section = true;
            if let Some(section) = current.take() {
                sections.push(section);
            }
            current = Some(Section {
                name,
                blank_after_heading: 0,
                entries: Vec::new(),
                blank_after_entries: 0,
            });
            seen_entry_in_current = false;
            continue;
        }

        if !seen_section {
            header_lines.push(line.to_string());
            continue;
        }

        if line.is_empty() {
            // A blank before the first entry is the heading->content gap; a blank
            // after entries is the separator before the next heading (or trailing
            // EOF blanks). Both are preserved verbatim. (Entries are assumed
            // contiguous — a blank between two entries is attributed to the
            // trailing count, which is true for MEMORY.md's bullet lists.)
            if let Some(section) = current.as_mut() {
                if seen_entry_in_current {
                    section.blank_after_entries += 1;
                } else {
                    section.blank_after_heading += 1;
                }
            }
            continue;
        }

        if let Some(section) = current.as_mut() {
            section.entries.push(line.to_string());
            seen_entry_in_current = true;
        } else {
            return Err(anyhow!(
                "parser state invalid: content exists without a section"
            ));
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

    let last = doc.sections.len();
    for (index, section) in doc.sections.iter().enumerate() {
        lines.push(format!("## {}", section.name));
        lines.extend(std::iter::repeat_n(
            String::new(),
            section.blank_after_heading,
        ));
        for entry in &section.entries {
            lines.push(entry.clone());
        }
        // Trailing blanks reproduce the parsed separator/EOF spacing. If a
        // freshly appended section left none, still emit one blank so adjacent
        // sections don't run their heading onto the previous entry.
        let trailing = if section.blank_after_entries == 0 && index + 1 < last {
            1
        } else {
            section.blank_after_entries
        };
        lines.extend(std::iter::repeat_n(String::new(), trailing));
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
    use super::{parse_doc, render_doc};

    // A committed sample that mirrors the real MEMORY.md shape — a header block,
    // a heading followed by a blank line, then entries — plus sections with zero
    // and two blank lines after their heading to prove arbitrary heading->content
    // spacing round-trips faithfully. Using a fixture instead of the live
    // ~/.claude memory file keeps the test deterministic and immune to the real
    // file's formatting drifting underneath it.
    const SAMPLE: &str = include_str!("../tests/fixtures/memory_sample.md");

    #[test]
    fn round_trip_sample_is_byte_identical() {
        let parsed = parse_doc(SAMPLE).expect("should parse sample");
        let rendered = render_doc(&parsed);
        assert_eq!(rendered, SAMPLE, "round-trip must preserve file identity");
    }

    #[test]
    fn parse_render_parse_is_stable() {
        let parsed = parse_doc(SAMPLE).expect("should parse sample");
        let rendered = render_doc(&parsed);
        let reparsed = parse_doc(&rendered).expect("re-parse should succeed");
        assert_eq!(
            parsed, reparsed,
            "parse -> render -> parse should be stable"
        );
    }

    #[test]
    fn heading_to_content_spacing_is_preserved() {
        let parsed = parse_doc(SAMPLE).expect("should parse sample");
        let gap = |name: &str| {
            parsed
                .sections
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("section {name} should exist"))
                .blank_after_heading
        };
        assert_eq!(gap("Active Signals"), 1);
        assert_eq!(gap("Compact"), 0);
        assert_eq!(gap("Overflow"), 2);
    }

    #[test]
    fn multi_blank_separator_between_sections_round_trips() {
        let input = "# Index\n\n## A\n\n- a1\n\n\n## B\n\n- b1\n";
        let rendered = render_doc(&parse_doc(input).expect("should parse"));
        assert_eq!(rendered, input, "two-blank separator must survive");
    }

    #[test]
    fn trailing_blank_line_round_trips() {
        let input = "# Index\n\n## A\n\n- a1\n\n";
        let rendered = render_doc(&parse_doc(input).expect("should parse"));
        assert_eq!(rendered, input, "trailing blank line must survive");
    }

    #[test]
    fn header_without_blank_before_heading_is_not_forced() {
        let input = "# Index\n## A\n\n- a1\n";
        let rendered = render_doc(&parse_doc(input).expect("should parse"));
        assert_eq!(rendered, input, "must not inject a blank after the header");
    }
}
