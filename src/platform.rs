//! OS and shell helpers.

use std::process::Command;

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

/// Create a `Command` for the given program, handling Windows `.cmd`/`.bat` resolution.
///
/// On Unix, this is equivalent to `Command::new(program)`.
///
/// On Windows, tools installed via version managers like `fnm` are typically
/// `.cmd` or `.bat` wrapper scripts (e.g. `npm.cmd` rather than `npm.exe`).
/// Rust's `std::process::Command::new()` uses `CreateProcessW` which does NOT
/// search for `.cmd`/`.bat` extensions, so `Command::new("npm")` fails with
/// "program not found" even though `npm.cmd` exists on PATH.
///
/// To work around this, on Windows we always spawn through `cmd /C` which
/// performs proper PATHEXT resolution (`.COM;.EXE;.BAT;.CMD;...`).
pub fn create_command(program: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C");
        cmd.arg(program);
        cmd
    } else {
        Command::new(program)
    }
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
