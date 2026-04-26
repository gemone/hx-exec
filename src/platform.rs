//! OS and shell helpers.

use anyhow::Result;
use std::process::Command;

#[cfg(target_os = "windows")]
use anyhow::anyhow;
#[cfg(target_os = "windows")]
use std::ffi::OsString;
#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};

/// Canonical current OS: "windows" | "macos" | "linux" | other raw `std::env::consts::OS`.
pub fn current_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        std::env::consts::OS
    }
}

/// Normalize a user-supplied OS matcher to its canonical form.
///
/// Accepts aliases like `win`, `darwin`, `mac`. The special value `unix`
/// matches every non-Windows OS; `any` / `*` matches all.
pub fn normalize_os(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "windows" | "win" | "win32" | "win64" => "windows",
        "macos" | "mac" | "darwin" | "osx" => "macos",
        "linux" => "linux",
        "unix" => "unix",
        "any" | "*" | "" => "any",
        other => {
            // Leak once for 'static; rarely taken path.
            Box::leak(other.to_string().into_boxed_str())
        }
    }
}

/// Does the user-supplied `want` matcher match `current`?
pub fn os_matches(want: &str, current: &str) -> bool {
    let w = normalize_os(want);
    match w {
        "any" => true,
        "unix" => current != "windows",
        other => other == current,
    }
}

/// Build (program, arg_flag) for a known shell.
/// Returns `None` for unknown shells — caller should error.
pub fn shell_invocation(shell: &str) -> Option<(&'static str, &'static [&'static str])> {
    match shell.trim().to_ascii_lowercase().as_str() {
        "bash" => Some(("bash", &["-c"])),
        "sh" => Some(("sh", &["-c"])),
        "zsh" => Some(("zsh", &["-c"])),
        "fish" => Some(("fish", &["-c"])),
        "dash" => Some(("dash", &["-c"])),
        "pwsh" => Some(("pwsh", &["-NoProfile", "-Command"])),
        "powershell" => Some(("powershell", &["-NoProfile", "-Command"])),
        "cmd" | "cmd.exe" => Some(("cmd", &["/C"])),
        _ => None,
    }
}

pub fn create_command(program: &str) -> Result<Command> {
    #[cfg(target_os = "windows")]
    {
        return Ok(Command::new(resolve_windows_program(program)?));
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(Command::new(program))
    }
}

#[cfg(target_os = "windows")]
fn resolve_windows_program(program: &str) -> Result<PathBuf> {
    let path = Path::new(program);
    if path.extension().is_some() {
        return Ok(path.to_path_buf());
    }

    let has_separator = program.contains('\\') || program.contains('/');
    let exts = pathexts();

    if has_separator {
        return resolve_with_exts(path, &exts)
            .ok_or_else(|| anyhow!("program not found: {}", program));
    }

    for dir in search_dirs() {
        let candidate = dir.join(program);
        if let Some(found) = resolve_with_exts(&candidate, &exts) {
            return Ok(found);
        }
    }

    Err(anyhow!("program not found: {}", program))
}

#[cfg(target_os = "windows")]
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(current) = std::env::current_dir() {
        dirs.push(current);
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

#[cfg(target_os = "windows")]
fn resolve_with_exts(base: &Path, exts: &[OsString]) -> Option<PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }

    for ext in exts {
        let candidate = base.with_extension(trim_dot(ext));
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn pathexts() -> Vec<OsString> {
    let raw = std::env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
    raw.to_string_lossy()
        .split(';')
        .filter(|part| !part.is_empty())
        .map(OsString::from)
        .collect()
}

#[cfg(target_os = "windows")]
fn trim_dot(ext: &OsString) -> String {
    ext.to_string_lossy().trim_start_matches('.').to_string()
}

/// Check if a shell name is valid, returning an error message if not.
pub fn validate_shell(shell: &str) -> Result<(), String> {
    if shell_invocation(shell).is_none() {
        Err(format!(
            "unknown shell `{}` (supported: bash, sh, zsh, fish, dash, pwsh, powershell, cmd)",
            shell
        ))
    } else {
        Ok(())
    }
}
