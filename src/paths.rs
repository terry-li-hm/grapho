use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;

pub fn resolve_memory_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(home.join(".claude/projects/-Users-terry/memory/MEMORY.md"))
}

pub fn resolve_overflow_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(home.join("docs/solutions/memory-overflow.md"))
}

pub fn resolve_solutions_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    let dir = home.join("docs/solutions");
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to ensure parent directory exists: {}", parent.display()))?;
    }
    Ok(dir)
}
