use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};

pub fn to_posix_path(path: &Path) -> Result<String> {
  let s = path
    .to_str()
    .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8"))?;
  Ok(s.replace('\\', "/"))
}

pub fn normalize_vault_rel_path(input: &str) -> Result<String> {
  let trimmed = input.trim().replace('\\', "/").trim_start_matches('/').to_string();
  if trimmed.is_empty() {
    bail!("path is required");
  }
  validate_rel_path(&trimmed)?;
  Ok(trimmed)
}

pub fn normalize_folder_rel_path(input: &str) -> Result<String> {
  let mut trimmed = input.trim().replace('\\', "/").to_string();
  trimmed = trimmed.trim_start_matches("./").to_string();
  trimmed = trimmed.trim_start_matches('/').to_string();
  trimmed = trimmed.trim_end_matches('/').to_string();
  if trimmed.is_empty() {
    bail!("folder path is required");
  }
  validate_rel_path(&trimmed)?;
  Ok(trimmed)
}

fn validate_rel_path(posix: &str) -> Result<()> {
  if posix.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
    bail!("invalid path");
  }
  Ok(())
}

pub fn join_inside(root: &Path, rel_posix: &str) -> Result<PathBuf> {
  // Reject traversal early.
  validate_rel_path(rel_posix)?;

  let mut out = PathBuf::from(root);
  for part in rel_posix.split('/') {
    out.push(part);
  }

  // A secondary guard: ensure it doesn't contain ParentDir components
  for c in out.strip_prefix(root).unwrap_or(&out).components() {
    if matches!(c, Component::ParentDir) {
      bail!("invalid path");
    }
  }

  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalize_rejects_empty() {
    assert!(normalize_vault_rel_path("  ").is_err());
  }

  #[test]
  fn normalize_rejects_traversal() {
    assert!(normalize_vault_rel_path("../a.md").is_err());
    assert!(normalize_folder_rel_path("a/../b").is_err());
  }

  #[test]
  fn normalize_converts_backslashes() {
    assert_eq!(normalize_vault_rel_path(r#"a\b.md"#).unwrap(), "a/b.md");
  }
}

