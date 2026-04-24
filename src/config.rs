//! Config loading: `hx-exec.toml`.
//!
//! Two alias forms are supported:
//!
//! ```toml
//! # Single form (no OS/shell matching):
//! [alias.rust-analyzer]
//! cmd = "rust-analyzer"
//!
//! # Array-of-tables form — one variant per OS/shell:
//! [[alias.my-tool]]
//! os = "windows"
//! shell = "pwsh"
//! cmd = "Get-Content foo | Where-Object { $_ -match 'bar' }"
//!
//! [[alias.my-tool]]
//! os = "linux"
//! shell = "bash"
//! cmd = "cat foo | grep bar"
//!
//! [[alias.my-tool]]
//! # default fallback, matches any OS
//! command = "my-tool"
//! args = ["--stdio"]
//! ```

use crate::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform;
use crate::presets;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub alias: HashMap<String, AliasEntry>,
}

/// A single alias may be either a single table or an array-of-tables.
/// We accept both via an untagged enum and normalize to `Vec<Alias>`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AliasEntry {
    Multi(Vec<Alias>),
    Single(Alias),
}

impl AliasEntry {
    pub fn variants(&self) -> &[Alias] {
        match self {
            AliasEntry::Multi(v) => v.as_slice(),
            AliasEntry::Single(a) => std::slice::from_ref(a),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Alias {
    /// OS matcher: "windows" | "macos" | "linux" | "unix" | "any" | aliases.
    /// When omitted, matches any OS.
    #[serde(default)]
    pub os: Option<String>,

    /// Optional shell to run `cmd` through (e.g. "pwsh", "bash", "cmd").
    /// When set, `cmd` is passed as a single script string to that shell
    /// (so pipes / redirects / native syntax work).
    #[serde(default)]
    pub shell: Option<String>,

    /// One-liner: full command string.
    ///
    /// Without `shell`: tokenized with shell-words, then each token is expanded.
    /// With `shell`:   passed as the single script argument after expansion.
    #[serde(default)]
    pub cmd: Option<String>,

    /// Structured: explicit program. Cannot be combined with `shell`.
    #[serde(default)]
    pub command: Option<String>,

    /// Structured: arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Per-alias env/extra vars available to expansion.
    /// Values are themselves expanded before use. These are both exported
    /// to the child process and made available to `${VAR}` expansion in
    /// `cmd` / `command` / `args`.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl Alias {
    /// Validate combination of fields early with a clear error.
    pub fn validate(&self) -> Result<()> {
        let has_cmd = self.cmd.is_some();
        let has_command = self.command.is_some();
        if !has_cmd && !has_command {
            return Err("alias must define either `cmd` or `command`".into());
        }
        if has_cmd && has_command {
            return Err("alias cannot define both `cmd` and `command`; choose one".into());
        }
        if self.shell.is_some() && has_command {
            return Err(
                "alias.shell requires `cmd` (script string), not `command` + `args`".into(),
            );
        }
        if let Some(shell) = &self.shell {
            if platform::shell_invocation(shell).is_none() {
                return Err(format!(
                    "unknown shell `{}` (supported: bash, sh, zsh, fish, dash, pwsh, powershell, cmd)",
                    shell
                )
                .into());
            }
        }
        Ok(())
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading config {}: {}", path.display(), e))?;
        let cfg: Config = toml::from_str(&text)
            .map_err(|e| format!("parsing TOML {}: {}", path.display(), e))?;
        Ok(cfg)
    }

    /// Resolve an alias for the current OS. Variants are tried in declared order;
    /// the first whose `os` matches (or is omitted) wins.
    pub fn resolve_alias(&self, name: &str) -> Result<&Alias> {
        let entry = self
            .alias
            .get(name)
            .ok_or_else(|| format!("alias not found: `{}`", name))?;
        let current = platform::current_os();
        let variants = entry.variants();

        // Prefer variants that explicitly match this OS over ones without `os`.
        let mut fallback: Option<&Alias> = None;
        for v in variants {
            match &v.os {
                Some(os) if platform::os_matches(os, current) => {
                    v.validate()?;
                    return Ok(v);
                }
                None => {
                    if fallback.is_none() {
                        fallback = Some(v);
                    }
                }
                Some(_) => {} // mismatched OS, skip
            }
        }
        if let Some(v) = fallback {
            v.validate()?;
            return Ok(v);
        }
        Err(format!("alias `{}` has no variant matching OS `{}`", name, current).into())
    }
}

/// Resolve which config file to load.
pub fn find_config(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("hx-exec.toml");
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(helix) = presets::helix_config_dir() {
        let p = helix.join("hx-exec.toml");
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(cfg) = presets::config_dir() {
        let p = cfg.join("hx-exec").join("hx-exec.toml");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_form() {
        let s = r#"
            [alias.foo]
            cmd = "echo hi"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        let a = cfg.resolve_alias("foo").unwrap();
        assert_eq!(a.cmd.as_deref(), Some("echo hi"));
    }

    #[test]
    fn parses_multi_form_and_picks_by_os() {
        let s = r#"
            [[alias.t]]
            os = "windows"
            cmd = "win-thing"

            [[alias.t]]
            os = "linux"
            cmd = "linux-thing"

            [[alias.t]]
            os = "macos"
            cmd = "mac-thing"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        let a = cfg.resolve_alias("t").unwrap();
        let expected = match platform::current_os() {
            "windows" => "win-thing",
            "linux" => "linux-thing",
            "macos" => "mac-thing",
            _ => panic!("unsupported test OS"),
        };
        assert_eq!(a.cmd.as_deref(), Some(expected));
    }

    #[test]
    fn falls_back_when_no_os_matches() {
        let s = r#"
            [[alias.t]]
            os = "plan9"
            cmd = "nope"

            [[alias.t]]
            cmd = "default"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        let a = cfg.resolve_alias("t").unwrap();
        assert_eq!(a.cmd.as_deref(), Some("default"));
    }

    #[test]
    fn unix_matcher_matches_non_windows() {
        let s = r#"
            [[alias.t]]
            os = "unix"
            cmd = "unix-thing"

            [[alias.t]]
            cmd = "fallback"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        let a = cfg.resolve_alias("t").unwrap();
        let expected = if cfg!(target_os = "windows") {
            "fallback"
        } else {
            "unix-thing"
        };
        assert_eq!(a.cmd.as_deref(), Some(expected));
    }

    #[test]
    fn rejects_shell_with_command() {
        let s = r#"
            [alias.bad]
            shell = "bash"
            command = "foo"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        assert!(cfg.resolve_alias("bad").is_err());
    }

    #[test]
    fn rejects_unknown_shell() {
        let s = r#"
            [alias.bad]
            shell = "zxyq"
            cmd = "foo"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        assert!(cfg.resolve_alias("bad").is_err());
    }

    #[test]
    fn no_matching_variant_errors() {
        let s = r#"
            [[alias.t]]
            os = "plan9"
            cmd = "nope"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        assert!(cfg.resolve_alias("t").is_err());
    }
}
