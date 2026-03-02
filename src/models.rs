use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub entries: Vec<String>,
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
