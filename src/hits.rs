use anyhow::{Context, Result};
use chrono::Local;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct HitRecord {
    pub count: u32,
    pub last: String,
}

pub type HitStore = HashMap<String, HitRecord>;

pub fn entry_key(text: &str) -> String {
    let stripped = text.trim().strip_prefix("- ").unwrap_or(text.trim());
    stripped.to_ascii_lowercase().chars().take(60).collect()
}

pub fn load_hits(path: &Path) -> Result<HitStore> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub fn save_hits(path: &Path, store: &HitStore) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent directory: {}", parent.display()))?;
    }

    let payload = serde_json::to_string_pretty(store).context("failed to serialize hits")?;
    let tmp_path = tmp_path(path);
    fs::write(&tmp_path, payload)
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

pub fn increment(store: &mut HitStore, key: &str) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let record = store.entry(key.to_string()).or_insert(HitRecord {
        count: 0,
        last: today.clone(),
    });
    record.count += 1;
    record.last = today;
}

fn tmp_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.tmp", path.display()))
}
