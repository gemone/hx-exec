//! Build and execute the final command.

use crate::Result;
use std::collections::HashMap;
use std::process::Command;

use crate::config::Alias;
use crate::expand::Expander;
use crate::platform;

/// Fully resolved command, ready to exec / print.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub program: String,
    pub args: Vec<String>,
    /// Extra env vars to set in the child process.
    pub env: HashMap<String, String>,
}

impl Resolved {
    /// Build from an alias: expand env first, then program and args using
    /// an expander that includes those env vars. When `alias.shell` is set,
    /// the resolved `cmd` is passed as a single script argument to that shell.
    pub fn from_alias(alias: &Alias) -> Result<Self> {
        alias.validate()?;

        // Expand per-alias env first (env values may reference process env / presets
        // but not each other, to keep semantics simple and predictable).
        let base = Expander::new();
        let mut extra: HashMap<String, String> = HashMap::new();
        for (k, v) in &alias.env {
            let ev = base
                .expand(v)
                .map_err(|e| format!("expanding env `{}`: {}", k, e))?;
            extra.insert(k.clone(), ev);
        }
        let expander = Expander::with_extra(extra.clone());

        // Shell-wrapped form: `<shell> <flags...> "<expanded-cmd>"`.
        if let Some(shell) = &alias.shell {
            let (prog, flags) = platform::shell_invocation(shell)
                .ok_or_else(|| format!("unknown shell: {}", shell))?;
            let raw = alias
                .cmd
                .as_deref()
                .ok_or("alias.shell requires `cmd`")?;
            let script = expander.expand_braced_only(raw)?;
            let mut args: Vec<String> = flags.iter().map(|s| s.to_string()).collect();
            args.push(script);
            return Ok(Self {
                program: prog.to_string(),
                args,
                env: extra,
            });
        }

        // Non-shell form: tokenize + expand each token individually.
        let (program_raw, args_raw) = tokens(alias)?;
        let program = expander.expand(&program_raw)?;
        let args = expander.expand_all(&args_raw)?;
        Ok(Self {
            program,
            args,
            env: extra,
        })
    }

    /// Build from a raw argv (already split), expanding every element.
    pub fn from_argv(argv: &[String]) -> Result<Self> {
        if argv.is_empty() {
            return Err("no command provided".into());
        }
        let expander = Expander::new();
        let program = expander.expand(&argv[0])?;
        let args = expander.expand_all(&argv[1..])?;
        Ok(Self {
            program,
            args,
            env: HashMap::new(),
        })
    }

    /// Append extra positional args (already expanded) to the resolved command.
    pub fn push_extra_args(&mut self, extra: &[String]) -> Result<()> {
        let expander = Expander::with_extra(self.env.clone());
        for a in extra {
            self.args.push(expander.expand(a)?);
        }
        Ok(())
    }

    /// Pretty print for `--print`.
    pub fn display(&self) -> String {
        let mut parts = Vec::with_capacity(self.args.len() + 1);
        parts.push(shell_words::quote(&self.program).into_owned());
        for a in &self.args {
            parts.push(shell_words::quote(a).into_owned());
        }
        parts.join(" ")
    }

    /// Execute, inheriting stdio. Returns exit code.
    pub fn exec(self) -> Result<i32> {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        let status = cmd.status().map_err(|e| {
            format!("failed to spawn `{}`: {}", self.program, e)
        })?;
        Ok(status.code().unwrap_or(if status.success() { 0 } else { 1 }))
    }
}

fn tokens(alias: &Alias) -> Result<(String, Vec<String>)> {
    if let Some(cmd) = &alias.cmd {
        let parts = shell_words::split(cmd)
            .map_err(|e| format!("tokenizing alias.cmd: {}", e))?;
        let mut it = parts.into_iter();
        let program = it
            .next()
            .ok_or("alias.cmd is empty")?;
        Ok((program, it.collect()))
    } else if let Some(command) = &alias.command {
        Ok((command.clone(), alias.args.clone()))
    } else {
        Err("alias must define either `cmd` or `command`".into())
    }
}
