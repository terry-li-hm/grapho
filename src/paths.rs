use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;

fn project_slug(home: &std::path::Path) -> Result<String> {
    let s = home
        .to_str()
        .ok_or_else(|| anyhow!("home path not utf-8"))?;
    Ok(s.replace('/', "-"))
}

pub fn resolve_memory_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    let slug = project_slug(&home)?;
    Ok(home.join(format!(".claude/projects/{}/memory/MEMORY.md", slug)))
}

pub fn resolve_overflow_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(home.join("docs/solutions/memory-overflow.md"))
}

pub fn resolve_solutions_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    let dir = home.join("docs/solutions");
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to ensure parent directory exists: {}",
                parent.display()
            )
        })?;
    }
    Ok(dir)
}

pub fn resolve_hits_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(home.join(".grapho/hits.json"))
}
