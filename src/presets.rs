use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Native directory helpers (replaces the `dirs` crate)
// ---------------------------------------------------------------------------

/// Return the current user's home directory.
///
/// * Windows : `%USERPROFILE%`
/// * Unix    : `$HOME`
fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Return the platform config base directory (without a sub-folder).
///
/// * Windows : `%APPDATA%`
/// * Linux   : `$XDG_CONFIG_HOME` or `$HOME/.config`
/// * macOS   : `$HOME/Library/Application Support`
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".config")))
    }
}

/// Return the platform cache base directory (without a sub-folder).
///
/// * Windows : `%LOCALAPPDATA%`
/// * Linux   : `$XDG_CACHE_HOME` or `$HOME/.cache`
/// * macOS   : `$HOME/Library/Caches`
fn cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Caches"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".cache")))
    }
}

// ---------------------------------------------------------------------------
// Helix-specific paths
// ---------------------------------------------------------------------------

/// Return Helix user config directory. Pure function of the OS:
///
/// * Linux  : `$XDG_CONFIG_HOME/helix` or `~/.config/helix`
/// * macOS  : `~/.config/helix` (Helix uses XDG-style even on macOS)
/// * Windows: `%AppData%\helix`
///
/// Note: we intentionally do NOT read a `HELIX_CONFIG` env var here —
/// `${HELIX_CONFIG}` in hx-exec is a preset that always points at the
/// user's helix directory. If you need to override for a specific alias,
/// set it in that alias's `env` table (alias.env has higher priority
/// than presets in the expansion lookup order).
pub fn helix_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = config_dir() {
            return Some(appdata.join("helix"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(xdg).join("helix"));
        }
        if let Some(home) = home_dir() {
            return Some(home.join(".config").join("helix"));
        }
    }

    None
}

/// Return Helix runtime directory. Honors `$HELIX_RUNTIME` (which Helix
/// itself reads), falling back to `${HELIX_CONFIG}/runtime`.
pub fn helix_runtime_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("HELIX_RUNTIME") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    helix_config_dir().map(|c| c.join("runtime"))
}

/// Return Helix cache dir.
///
/// * Linux  : `$XDG_CACHE_HOME/helix` or `~/.cache/helix`
/// * macOS  : `~/Library/Caches/helix`
/// * Windows: `%LocalAppData%\helix`
pub fn helix_cache_dir() -> Option<PathBuf> {
    cache_dir().map(|c| c.join("helix"))
}

/// Resolve a preset name to its string value, if any.
pub fn resolve(name: &str) -> Option<String> {
    let path = match name {
        "HELIX_CONFIG" => helix_config_dir(),
        "HELIX_RUNTIME" => helix_runtime_dir(),
        "HELIX_CACHE" => helix_cache_dir(),
        _ => return None,
    };
    path.map(|p| p.to_string_lossy().into_owned())
}
