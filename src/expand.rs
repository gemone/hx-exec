//! String expansion: `${VAR}`, `$VAR`, and `$(cmd ...)`.
//!
//! Expansion precedence for variables:
//! 1. Extra variables passed in (e.g. alias-provided env)
//! 2. `presets::resolve` (HELIX_CONFIG, HELIX_RUNTIME, HELIX_CACHE)
//! 3. Process environment
//!
//! Command substitution `$(...)` is parsed via `shell-words` and executed
//! directly (no shell), for cross-platform consistency. stdout is captured
//! and trailing whitespace/newlines are trimmed.

use crate::Result;
use std::collections::HashMap;
use std::process::Command;

use crate::presets;

#[derive(Default, Debug, Clone)]
pub struct Expander {
    pub extra: HashMap<String, String>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Mode {
    Full,
    BracedOnly,
}

impl Expander {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_extra(extra: HashMap<String, String>) -> Self {
        Self { extra }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.extra.insert(key.into(), value.into());
    }

    /// Expand one string, returning the result.
    pub fn expand(&self, input: &str) -> Result<String> {
        self.expand_with(input, Mode::Full)
    }

    /// Expand only `${VAR}` (braced) form, leaving `$VAR` and `$(cmd)` as-is.
    /// Use this when the result will be processed further by a shell.
    pub fn expand_braced_only(&self, input: &str) -> Result<String> {
        self.expand_with(input, Mode::BracedOnly)
    }

    fn expand_with(&self, input: &str, mode: Mode) -> Result<String> {
        let mut out = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            if c == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'$' {
                // Escaped dollar: emit literal '$'
                out.push('$');
                i += 2;
                continue;
            }
            if c == b'$' && i + 1 < bytes.len() {
                match bytes[i + 1] {
                    b'(' if mode == Mode::Full => {
                        let end = find_matching(bytes, i + 1, b'(', b')')
                            .ok_or_else(|| {
                                format!("unterminated $( in: {}", input)
                            })?;
                        let inner = std::str::from_utf8(&bytes[i + 2..end])?;
                        // Allow nested expansion inside $(...)
                        let expanded_cmd = self.expand(inner)?;
                        let output = run_capture(&expanded_cmd).map_err(|e| {
                            format!("command substitution failed: $({}): {}", inner, e)
                        })?;
                        out.push_str(&output);
                        i = end + 1;
                        continue;
                    }
                    b'{' => {
                        let end = find_matching(bytes, i + 1, b'{', b'}')
                            .ok_or_else(|| {
                                format!("unterminated ${{ in: {}", input)
                            })?;
                        let name = std::str::from_utf8(&bytes[i + 2..end])?;
                        out.push_str(&self.lookup(name));
                        i = end + 1;
                        continue;
                    }
                    b if is_var_start(b) && mode == Mode::Full => {
                        let start = i + 1;
                        let mut j = start;
                        while j < bytes.len() && is_var_cont(bytes[j]) {
                            j += 1;
                        }
                        let name = std::str::from_utf8(&bytes[start..j])?;
                        out.push_str(&self.lookup(name));
                        i = j;
                        continue;
                    }
                    _ => {}
                }
            }
            out.push(c as char);
            i += 1;
        }
        Ok(out)
    }

    /// Expand each element of a slice.
    pub fn expand_all<S: AsRef<str>>(&self, items: &[S]) -> Result<Vec<String>> {
        items.iter().map(|s| self.expand(s.as_ref())).collect()
    }

    fn lookup(&self, name: &str) -> String {
        if let Some(v) = self.extra.get(name) {
            return v.clone();
        }
        if let Some(v) = presets::resolve(name) {
            return v;
        }
        std::env::var(name).unwrap_or_default()
    }
}

fn is_var_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}
fn is_var_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Given bytes and position of the opener (e.g. the `(` or `{`), find the
/// index of the matching closer, honoring nesting.
fn find_matching(bytes: &[u8], open_idx: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 1i32;
    let mut i = open_idx + 1;
    while i < bytes.len() {
        let c = bytes[i];
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Run a command string (no shell), capture stdout, trim trailing whitespace.
fn run_capture(cmd: &str) -> Result<String> {
    let parts = shell_words::split(cmd)
        .map_err(|e| format!("failed to tokenize command: {}", e))?;
    if parts.is_empty() {
        return Ok(String::new());
    }
    let (program, args) = parts.split_first().unwrap();
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn {}: {}", program, e))?;
    if !output.status.success() {
        return Err(format!(
            "`{}` exited with {}: {}",
            cmd,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let mut s = String::from_utf8(output.stdout)
        .map_err(|e| format!("command stdout was not UTF-8: {}", e))?;
    while s.ends_with(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t') {
        s.pop();
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braced_var() {
        std::env::set_var("HX_TEST_FOO", "bar");
        let e = Expander::new();
        assert_eq!(e.expand("x=${HX_TEST_FOO}/y").unwrap(), "x=bar/y");
    }

    #[test]
    fn bare_var() {
        std::env::set_var("HX_TEST_BAZ", "qux");
        let e = Expander::new();
        assert_eq!(e.expand("$HX_TEST_BAZ!").unwrap(), "qux!");
    }

    #[test]
    fn extra_beats_env() {
        std::env::set_var("HX_TEST_X", "env");
        let mut e = Expander::new();
        e.set("HX_TEST_X", "extra");
        assert_eq!(e.expand("${HX_TEST_X}").unwrap(), "extra");
    }

    #[test]
    fn missing_var_is_empty() {
        let e = Expander::new();
        assert_eq!(e.expand("[${HX_MISSING_123}]").unwrap(), "[]");
    }

    #[test]
    fn escaped_dollar() {
        let e = Expander::new();
        assert_eq!(e.expand(r"\${NOPE}").unwrap(), "${NOPE}");
    }

    #[test]
    fn command_substitution() {
        let e = Expander::new();
        // `echo` is available on all platforms we target via the binary path.
        #[cfg(not(target_os = "windows"))]
        let out = e.expand("hello $(echo world)").unwrap();
        #[cfg(target_os = "windows")]
        let out = e.expand("hello $(cmd /C echo world)").unwrap();
        assert!(out.starts_with("hello "));
        assert!(out.contains("world"));
    }

    #[test]
    fn nested_expansion_in_cmdsub() {
        std::env::set_var("HX_TEST_MSG", "hi");
        let e = Expander::new();
        #[cfg(not(target_os = "windows"))]
        let out = e.expand("$(echo ${HX_TEST_MSG})").unwrap();
        #[cfg(target_os = "windows")]
        let out = e.expand("$(cmd /C echo ${HX_TEST_MSG})").unwrap();
        assert!(out.contains("hi"));
    }

    #[test]
    fn braced_only_preserves_native_shell_syntax() {
        std::env::set_var("HX_TEST_V", "vv");
        let e = Expander::new();
        let out = e
            .expand_braced_only("pre ${HX_TEST_V} $HX_TEST_V $(uname)")
            .unwrap();
        // ${...} expanded; $VAR and $(...) passed through verbatim.
        assert_eq!(out, "pre vv $HX_TEST_V $(uname)");
    }

    #[test]
    fn helix_config_preset_ignores_same_named_env_var() {
        // Setting a HELIX_CONFIG env var must NOT change the preset result:
        // ${HELIX_CONFIG} is always the detected helix directory.
        let before = crate::presets::helix_config_dir();
        std::env::set_var("HELIX_CONFIG", "/tmp/hx-exec-bogus-path-should-be-ignored");
        let after = crate::presets::helix_config_dir();
        std::env::remove_var("HELIX_CONFIG");
        assert_eq!(before, after);
        assert_ne!(
            after.as_ref().map(|p| p.to_string_lossy().into_owned()),
            Some("/tmp/hx-exec-bogus-path-should-be-ignored".to_string())
        );
    }

    #[test]
    fn helix_config_preset_present() {
        let e = Expander::new();
        let out = e.expand("${HELIX_CONFIG}").unwrap();
        assert!(!out.is_empty(), "HELIX_CONFIG should resolve");
    }
}
