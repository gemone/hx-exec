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

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform;
use crate::presets;

/// The value of a per-alias env entry.
///
/// Two forms are supported in TOML:
///
/// ```toml
/// # Literal string (existing behaviour):
/// env.FOO = "bar"
///
/// # Command whose stdout becomes the value:
/// env.NPM_ROOT = { cmd = "npm root -g" }
///
/// # Same, but run through a specific shell:
/// env.NPM_ROOT = { cmd = "npm root -g", shell = "pwsh" }
/// ```
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum EnvValue {
    /// A literal string value.
    Literal(String),
    /// Run a command and use its stdout (trimmed) as the value.
    Command(EnvCommand),
}

/// Specification for a command-derived env value.
#[derive(Debug, Deserialize, Clone)]
pub struct EnvCommand {
    /// The command to execute. Without `shell`, tokenized via shell-words and
    /// run directly (cross-platform). With `shell`, passed as a single script
    /// argument to that shell.
    pub cmd: String,
    /// Optional shell to run `cmd` through (same names as `alias.shell`).
    #[serde(default)]
    pub shell: Option<String>,
}

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
    /// Values may be literal strings or command-derived (see [`EnvValue`]).
    /// Resolved values are exported to the child process and made available
    /// to `${VAR}` expansion in `cmd` / `command` / `args`.
    #[serde(default)]
    pub env: HashMap<String, EnvValue>,
}

impl Alias {
    /// Validate combination of fields early with a clear error.
    pub fn validate(&self) -> Result<()> {
        let has_cmd = self.cmd.is_some();
        let has_command = self.command.is_some();
        if !has_cmd && !has_command {
            return Err(anyhow!("alias must define either `cmd` or `command`"));
        }
        if has_cmd && has_command {
            return Err(anyhow!(
                "alias cannot define both `cmd` and `command`; choose one"
            ));
        }
        if self.shell.is_some() && has_command {
            return Err(anyhow!(
                "alias.shell requires `cmd` (script string), not `command` + `args`"
            ));
        }
        if let Some(shell) = &self.shell {
            if platform::shell_invocation(shell).is_none() {
                return Err(anyhow!(
                    "unknown shell `{}` (supported: bash, sh, zsh, fish, dash, pwsh, powershell, cmd)",
                    shell
                ));
            }
        }
        // Validate shell names inside command-derived env values.
        for (k, v) in &self.env {
            if let EnvValue::Command(ec) = v {
                if let Some(shell) = &ec.shell {
                    if platform::shell_invocation(shell).is_none() {
                        return Err(anyhow!(
                            "env `{}` has unknown shell `{}` (supported: bash, sh, zsh, fish, dash, pwsh, powershell, cmd)",
                            k,
                            shell
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config: {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing TOML: {}", path.display()))?;
        Ok(cfg)
    }

    /// Resolve an alias for the current OS. Variants are tried in declared order;
    /// the first whose `os` matches (or is omitted) wins.
    pub fn resolve_alias(&self, name: &str) -> Result<&Alias> {
        let entry = self
            .alias
            .get(name)
            .ok_or_else(|| anyhow!("alias not found: `{}`", name))?;
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
        Err(anyhow!(
            "alias `{}` has no variant matching OS `{}`",
            name,
            current
        ))
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
    if let Some(cfg) = dirs::config_dir() {
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

    #[test]
    fn env_literal_value_parses() {
        let s = r#"
            [alias.t]
            cmd = "echo hi"
            env.FOO = "bar"
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        let a = cfg.resolve_alias("t").unwrap();
        match a.env.get("FOO").unwrap() {
            EnvValue::Literal(v) => assert_eq!(v, "bar"),
            _ => panic!("expected Literal"),
        }
    }

    #[test]
    fn env_command_value_parses() {
        let s = r#"
            [alias.t]
            cmd = "echo hi"
            env.MY_VAR = { cmd = "echo hello" }
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        let a = cfg.resolve_alias("t").unwrap();
        match a.env.get("MY_VAR").unwrap() {
            EnvValue::Command(ec) => {
                assert_eq!(ec.cmd, "echo hello");
                assert!(ec.shell.is_none());
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn env_command_with_shell_parses() {
        let s = r#"
            [alias.t]
            cmd = "echo hi"
            env.MY_VAR = { cmd = "Write-Output hello", shell = "pwsh" }
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        let a = cfg.resolve_alias("t").unwrap();
        match a.env.get("MY_VAR").unwrap() {
            EnvValue::Command(ec) => {
                assert_eq!(ec.cmd, "Write-Output hello");
                assert_eq!(ec.shell.as_deref(), Some("pwsh"));
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn env_command_rejects_unknown_shell() {
        let s = r#"
            [alias.t]
            cmd = "echo hi"
            env.MY_VAR = { cmd = "something", shell = "unknownsh" }
        "#;
        let cfg: Config = toml::from_str(s).unwrap();
        assert!(cfg.resolve_alias("t").is_err());
    }
}
