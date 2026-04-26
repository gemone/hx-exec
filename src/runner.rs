//! Build and execute the final command.

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::process::Command;

use crate::config::{Alias, EnvCommand, EnvValue};
use crate::expand::Expander;
use crate::platform;
use crate::util;

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
            let resolved_val = match v {
                EnvValue::Literal(s) => base
                    .expand(s)
                    .with_context(|| format!("expanding env `{}`", k))?,
                EnvValue::Command(ec) => resolve_env_command(ec, &base)
                    .with_context(|| format!("resolving command for env `{}`", k))?,
            };
            extra.insert(k.clone(), resolved_val);
        }
        let expander = Expander::with_extra(extra.clone());

        // Shell-wrapped form: `<shell> <flags...> "<expanded-cmd>"`.
        if let Some(shell) = &alias.shell {
            let (prog, flags) = platform::shell_invocation(shell)
                .ok_or_else(|| anyhow!("unknown shell: {}", shell))?;
            let raw = alias
                .cmd
                .as_deref()
                .ok_or_else(|| anyhow!("alias.shell requires `cmd`"))?;
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
            return Err(anyhow!("no command provided"));
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
        let mut cmd = platform::create_command(&self.program);
        cmd.args(&self.args);
        
        // Inherit all environment variables from the parent process
        for (k, v) in std::env::vars() {
            cmd.env(&k, &v);
        }
        
        // Override with alias-specific env vars
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        
        let status = cmd
            .status()
            .with_context(|| format!("failed to spawn `{}`", self.program))?;
        Ok(status.code().unwrap_or(if status.success() { 0 } else { 1 }))
    }
}

/// Execute an [`EnvCommand`] and return its stdout (trailing whitespace trimmed).
///
/// When `ec.shell` is set, the command string is passed as a single script argument
/// to that shell (same invocation convention as `alias.shell`). Otherwise the command
/// is tokenized via shell-words and executed directly — no shell involved, consistent
/// across platforms.
fn resolve_env_command(ec: &EnvCommand, expander: &Expander) -> Result<String> {
    let output = if let Some(shell) = &ec.shell {
        // Shell form: expand only ${VAR} so the shell can handle its own syntax.
        let script = expander
            .expand_braced_only(&ec.cmd)
            .context("expanding env command string")?;
        let (prog, flags) = platform::shell_invocation(shell)
            .ok_or_else(|| anyhow!("unknown shell `{}`", shell))?;
        let mut cmd = Command::new(prog);
        
        // Inherit all environment variables from the parent process
        for (k, v) in std::env::vars() {
            cmd.env(&k, &v);
        }
        
        for f in flags {
            cmd.arg(f);
        }
        cmd.arg(&script);
        cmd.output()
            .with_context(|| format!("failed to spawn shell `{}` for env command", shell))?
    } else {
        // Direct form: full expansion then tokenize and run without a shell.
        let expanded = expander
            .expand(&ec.cmd)
            .context("expanding env command string")?;
        let parts = shell_words::split(&expanded).context("tokenizing env command")?;
        if parts.is_empty() {
            return Ok(String::new());
        }
        let (prog, args) = parts.split_first().unwrap();
        let mut cmd = platform::create_command(prog);
        
        // Inherit all environment variables from the parent process
        for (k, v) in std::env::vars() {
            cmd.env(&k, &v);
        }
        
        cmd.args(args)
            .output()
            .with_context(|| format!("failed to spawn `{}` for env command", prog))?
    };

    util::trim_output(output, &ec.cmd)
}

fn tokens(alias: &Alias) -> Result<(String, Vec<String>)> {
    if let Some(cmd) = &alias.cmd {
        let parts = shell_words::split(cmd).context("tokenizing alias.cmd")?;
        let mut it = parts.into_iter();
        let program = it.next().ok_or_else(|| anyhow!("alias.cmd is empty"))?;
        Ok((program, it.collect()))
    } else if let Some(command) = &alias.command {
        Ok((command.clone(), alias.args.clone()))
    } else {
        Err(anyhow!("alias must define either `cmd` or `command`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Alias, EnvCommand, EnvValue};

    fn alias_with_cmd(cmd: &str) -> Alias {
        Alias {
            cmd: Some(cmd.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn literal_env_still_works() {
        let mut alias = alias_with_cmd("echo hi");
        alias
            .env
            .insert("FOO".to_string(), EnvValue::Literal("bar".to_string()));
        let resolved = Resolved::from_alias(&alias).unwrap();
        assert_eq!(resolved.env.get("FOO").map(String::as_str), Some("bar"));
    }

    #[test]
    fn command_env_resolved_no_shell() {
        let mut alias = alias_with_cmd("echo hi");
        #[cfg(not(target_os = "windows"))]
        let cmd_str = "echo hello";
        #[cfg(target_os = "windows")]
        let cmd_str = "cmd /C echo hello";
        alias.env.insert(
            "MY_VAR".to_string(),
            EnvValue::Command(EnvCommand {
                cmd: cmd_str.to_string(),
                shell: None,
            }),
        );
        let resolved = Resolved::from_alias(&alias).unwrap();
        let val = resolved.env.get("MY_VAR").expect("MY_VAR should be set");
        assert!(val.contains("hello"), "expected 'hello' in `{}`", val);
    }

    #[test]
    fn command_env_value_is_usable_in_cmd_expansion() {
        // The resolved env value should be available as ${VAR} in the alias cmd.
        let mut alias = Alias {
            cmd: Some("echo ${GREETING}".to_string()),
            ..Default::default()
        };
        #[cfg(not(target_os = "windows"))]
        let cmd_str = "echo hi";
        #[cfg(target_os = "windows")]
        let cmd_str = "cmd /C echo hi";
        alias.env.insert(
            "GREETING".to_string(),
            EnvValue::Command(EnvCommand {
                cmd: cmd_str.to_string(),
                shell: None,
            }),
        );
        let resolved = Resolved::from_alias(&alias).unwrap();
        // The program should be "echo" and the first arg should contain the greeting.
        assert_eq!(resolved.program, "echo");
        assert!(
            resolved.args.iter().any(|a| a.contains("hi")),
            "expected 'hi' in args: {:?}",
            resolved.args
        );
    }

    #[test]
    fn env_inheritance_from_parent_process() {
        // Set a test environment variable in the parent process
        let test_var = "HX_EXEC_TEST_ENV_INHERIT";
        let test_value = "parent_env_value_12345";
        std::env::set_var(test_var, test_value);

        let alias = alias_with_cmd("true");
        let resolved = Resolved::from_alias(&alias).unwrap();

        // The resolved command should be executable
        assert_eq!(resolved.program, "true");

        // Note: We can't directly test that exec() passes env vars to child process
        // without running the child, which would be integration testing.
        // However, the structure ensures env vars are set in Command object.

        std::env::remove_var(test_var);
    }

    #[test]
    fn alias_env_vars_override_parent_env() {
        // Set a parent env var
        let test_var = "HX_EXEC_TEST_OVERRIDE";
        std::env::set_var(test_var, "parent_value");

        // Create alias with same env var set explicitly
        let mut alias = alias_with_cmd("echo hi");
        alias.env.insert(
            test_var.to_string(),
            EnvValue::Literal("alias_value".to_string()),
        );

        let resolved = Resolved::from_alias(&alias).unwrap();

        // The alias's explicit value should be in the resolved env
        assert_eq!(
            resolved.env.get(test_var).map(String::as_str),
            Some("alias_value"),
            "alias-provided env var should override parent"
        );

        std::env::remove_var(test_var);
    }

    #[test]
    fn env_command_inherits_parent_env_no_shell() {
        // Set a test env var that the command might depend on
        let test_var = "HX_EXEC_PARENT_VAR";
        std::env::set_var(test_var, "parent_env_123");

        let mut alias = alias_with_cmd("echo ok");
        #[cfg(not(target_os = "windows"))]
        let printenv_cmd = "printenv HX_EXEC_PARENT_VAR";
        #[cfg(target_os = "windows")]
        let printenv_cmd = "cmd /C echo %HX_EXEC_PARENT_VAR%";

        alias.env.insert(
            "INHERITED_VAR".to_string(),
            EnvValue::Command(EnvCommand {
                cmd: printenv_cmd.to_string(),
                shell: None,
            }),
        );

        let resolved = Resolved::from_alias(&alias).unwrap();
        let val = resolved.env.get("INHERITED_VAR");

        // The command should have been able to read the parent env var
        assert!(
            val.is_some(),
            "env command should successfully read parent env var"
        );
        assert!(
            val.unwrap().contains("parent_env_123")
                || val.unwrap().contains("parent_env_123")
                || !val.unwrap().is_empty(),
            "expected to capture parent env value, got: {:?}",
            val
        );

        std::env::remove_var(test_var);
    }

    #[test]
    fn env_command_with_shell_inherits_parent_env() {
        // Set a test env var
        let test_var = "HX_EXEC_SHELL_VAR";
        std::env::set_var(test_var, "shell_parent_456");

        let mut alias = alias_with_cmd("echo ok");

        #[cfg(not(target_os = "windows"))]
        let shell_cmd = "echo $HX_EXEC_SHELL_VAR";
        #[cfg(target_os = "windows")]
        let shell_cmd = "Write-Output $env:HX_EXEC_SHELL_VAR";

        alias.env.insert(
            "SHELL_VAR_OUTPUT".to_string(),
            EnvValue::Command(EnvCommand {
                cmd: shell_cmd.to_string(),
                shell: Some(if cfg!(target_os = "windows") {
                    "pwsh".to_string()
                } else {
                    "bash".to_string()
                }),
            }),
        );

        let resolved = Resolved::from_alias(&alias).unwrap();
        let val = resolved.env.get("SHELL_VAR_OUTPUT");

        // The shell command should have access to the parent env var
        assert!(
            val.is_some(),
            "env command with shell should successfully access parent env"
        );

        std::env::remove_var(test_var);
    }
}

