//! Shared utility functions.

use anyhow::{anyhow, Context, Result};
use std::process::Output;

/// Capture and trim output from a Command result.
/// Trims trailing newlines, carriage returns, spaces, and tabs.
pub fn trim_output(output: Output, cmd: &str) -> Result<String> {
    if !output.status.success() {
        return Err(anyhow!(
            "`{}` exited with {}: {}",
            cmd,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let s = String::from_utf8(output.stdout).context("command stdout was not UTF-8")?;
    Ok(s.trim_end().to_string())
}
