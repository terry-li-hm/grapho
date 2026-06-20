use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;

fn project_slug(home: &std::path::Path) -> Result<String> {
    let s = home
        .to_str()
        .ok_or_else(|| anyhow!("home path not utf-8"))?;
    Ok(s.replace('/', "-"))
}

/// Expand a leading `~` to the home directory; leave any other path untouched.
fn expand_tilde(path: PathBuf) -> Result<PathBuf> {
    match path.strip_prefix("~") {
        Ok(rest) => {
            let home =
                dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
            Ok(home.join(rest))
        }
        Err(_) => Ok(path),
    }
}

/// Read a path-valued environment override, ignoring an unset or empty value.
/// Lets `grapho` be pointed at an alternate memory set (e.g. the epigenome
/// MEMORY.md) without rebuilding — the binary is otherwise hardwired.
fn env_path_override(var: &str) -> Result<Option<PathBuf>> {
    match std::env::var_os(var) {
        Some(raw) if !raw.is_empty() => Ok(Some(expand_tilde(PathBuf::from(raw))?)),
        _ => Ok(None),
    }
}

pub fn resolve_memory_path() -> Result<PathBuf> {
    if let Some(path) = env_path_override("GRAPHO_MEMORY_PATH")? {
        return Ok(path);
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    let slug = project_slug(&home)?;
    Ok(home.join(format!(".claude/projects/{}/memory/MEMORY.md", slug)))
}

pub fn resolve_memory_dir() -> Result<PathBuf> {
    let mem = resolve_memory_path()?;
    mem.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("memory path has no parent dir"))
}

pub fn resolve_marks_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(home.join("epigenome/marks"))
}

pub fn resolve_overflow_path() -> Result<PathBuf> {
    if let Some(path) = env_path_override("GRAPHO_OVERFLOW_PATH")? {
        return Ok(path);
    }
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
