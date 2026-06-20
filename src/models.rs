use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    /// Blank lines between the `## heading` and the section's first entry.
    /// Preserved so a round-trip doesn't collapse the canonical
    /// `## Heading\n\n- entry` spacing into `## Heading\n- entry`.
    pub blank_after_heading: usize,
    pub entries: Vec<String>,
    /// Blank lines after the last entry. These double as the separator before
    /// the next heading (or trailing blank lines at end of file), so preserving
    /// the count keeps inter-section spacing faithful on round-trip.
    pub blank_after_entries: usize,
}

impl Section {
    /// Create an empty section using the canonical one-blank-line gap between
    /// the heading and its entries — the shape MEMORY.md is written in.
    pub fn new(name: impl Into<String>) -> Self {
        Section {
            name: name.into(),
            blank_after_heading: 1,
            entries: Vec::new(),
            blank_after_entries: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryEntry {
    pub section: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct GraphoReport {
    pub memory_path: String,
    pub line_count: usize,
    pub budget: usize,
    pub remaining: i64,
    pub over_budget: bool,
    pub sections: Vec<String>,
    pub top_hits: Vec<HitEntry>,
}

#[derive(Debug, Serialize, Clone)]
pub struct HitEntry {
    pub key: String,
    pub count: u32,
    pub last: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct Orphan {
    pub name: String,
    pub path: String,
    pub age_days: i64,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct OrphansReport {
    pub memory_dir: String,
    pub marks_dir: String,
    pub min_age_days: i64,
    pub count: usize,
    pub orphans: Vec<Orphan>,
}
