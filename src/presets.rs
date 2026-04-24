use std::path::PathBuf;

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
        if let Some(appdata) = dirs::config_dir() {
            return Some(appdata.join("helix"));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("helix"));
            }
        }
        if let Some(home) = dirs::home_dir() {
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
    dirs::cache_dir().map(|c| c.join("helix"))
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
