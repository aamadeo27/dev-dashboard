// ProjectScanner — detects language and package manager from marker files.

use std::path::Path;

/// Detect the language and package manager for a project directory by
/// inspecting marker files in a fixed priority order.
///
/// Returns `(language, package_manager)`. Both are `None` if no marker is
/// recognized.
///
/// Detection order (first match wins):
/// 1. `Cargo.toml`                              → rust / cargo
/// 2. `package.json` + `pnpm-lock.yaml`         → typescript / pnpm
/// 3. `package.json` (alone)                    → typescript / npm
/// 4. `pyproject.toml` + `uv.lock`              → python / uv
/// 5. `pyproject.toml` + `poetry.lock`          → python / poetry
/// 6. `go.mod`                                  → go / gomod
/// 7. (none of the above)                       → (None, None)
///
/// Uses synchronous `Path::exists()` — this function is called outside the
/// async mutex and is acceptable for blocking use on the Tokio thread pool.
pub(crate) fn detect(path: &Path) -> (Option<String>, Option<String>) {
    if path.join("Cargo.toml").exists() {
        return (Some("rust".into()), Some("cargo".into()));
    }

    if path.join("package.json").exists() {
        if path.join("pnpm-lock.yaml").exists() {
            return (Some("typescript".into()), Some("pnpm".into()));
        }
        return (Some("typescript".into()), Some("npm".into()));
    }

    if path.join("pyproject.toml").exists() {
        if path.join("uv.lock").exists() {
            return (Some("python".into()), Some("uv".into()));
        }
        if path.join("poetry.lock").exists() {
            return (Some("python".into()), Some("poetry".into()));
        }
    }

    if path.join("go.mod").exists() {
        return (Some("go".into()), Some("gomod".into()));
    }

    (None, None)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(dir: &TempDir, filename: &str) {
        fs::write(dir.path().join(filename), b"").expect("touch file");
    }

    #[test]
    fn detect_rust_cargo_toml() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "Cargo.toml");
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang.as_deref(), Some("rust"));
        assert_eq!(pm.as_deref(), Some("cargo"));
    }

    #[test]
    fn detect_typescript_pnpm() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "package.json");
        touch(&dir, "pnpm-lock.yaml");
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang.as_deref(), Some("typescript"));
        assert_eq!(pm.as_deref(), Some("pnpm"));
    }

    #[test]
    fn detect_typescript_npm_without_pnpm_lock() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "package.json");
        // No pnpm-lock.yaml — should fall back to npm.
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang.as_deref(), Some("typescript"));
        assert_eq!(pm.as_deref(), Some("npm"));
    }

    #[test]
    fn detect_python_uv() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "pyproject.toml");
        touch(&dir, "uv.lock");
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang.as_deref(), Some("python"));
        assert_eq!(pm.as_deref(), Some("uv"));
    }

    #[test]
    fn detect_python_poetry() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "pyproject.toml");
        touch(&dir, "poetry.lock");
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang.as_deref(), Some("python"));
        assert_eq!(pm.as_deref(), Some("poetry"));
    }

    #[test]
    fn detect_go_gomod() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "go.mod");
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang.as_deref(), Some("go"));
        assert_eq!(pm.as_deref(), Some("gomod"));
    }

    #[test]
    fn detect_unknown_returns_none() {
        let dir = TempDir::new().unwrap();
        // No marker files present.
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang, None);
        assert_eq!(pm, None);
    }

    /// Cargo.toml takes priority over package.json when both are present.
    #[test]
    fn detect_cargo_takes_priority_over_package_json() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "Cargo.toml");
        touch(&dir, "package.json");
        let (lang, pm) = detect(dir.path());
        assert_eq!(
            lang.as_deref(),
            Some("rust"),
            "Cargo.toml must win over package.json"
        );
        assert_eq!(pm.as_deref(), Some("cargo"));
    }

    /// `pyproject.toml` without a recognised lock file must not match any stack.
    /// Python detection requires uv.lock or poetry.lock to avoid false-positives
    /// on non-Python projects that happen to ship a pyproject.toml.
    #[test]
    fn detect_pyproject_alone_returns_none() {
        let dir = TempDir::new().unwrap();
        touch(&dir, "pyproject.toml");
        // No uv.lock or poetry.lock.
        let (lang, pm) = detect(dir.path());
        assert_eq!(lang, None);
        assert_eq!(pm, None);
    }
}
