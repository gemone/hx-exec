use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

use hx_exec::{
    config::{find_config, Config},
    runner::Resolved,
};

/// Cross-platform launcher that expands `${VAR}`, `$(cmd)`, and TOML aliases.
///
/// Examples:
///   hx-exec -- ngserver --stdio --tsProbeLocations "$(npm -g root)"
///   hx-exec -c angular-lsp
///   hx-exec -c angular-lsp -- --extra-flag
///   hx-exec --print -- my-lsp --root "${HELIX_CONFIG}"
#[derive(Debug, Parser)]
#[command(
    name = "hx-exec",
    version,
    about = "Expand ${VAR} / $(cmd) and launch — built for Helix LSPs across OSes.",
    trailing_var_arg = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// Named alias from hx-exec.toml to execute.
    #[arg(short = 'c', long = "config", value_name = "ALIAS")]
    alias: Option<String>,

    /// Explicit path to an hx-exec.toml file.
    #[arg(short = 'f', long = "file", value_name = "PATH")]
    file: Option<PathBuf>,

    /// Print the fully resolved command instead of executing it.
    #[arg(long)]
    print: bool,

    /// The command (and args) to run. Everything after `--` is forwarded.
    /// When `-c` is given, these are appended to the alias's argv.
    #[arg(value_name = "CMD", num_args = 0..)]
    command: Vec<String>,
}

fn run() -> Result<i32> {
    let cli = Cli::parse();

    let mut resolved = if let Some(alias_name) = &cli.alias {
        let path = find_config(cli.file.as_deref()).ok_or_else(|| {
            anyhow!(
                "could not locate hx-exec.toml (looked at --file, ./hx-exec.toml, \
                 ${{HELIX_CONFIG}}/hx-exec.toml, and <config>/hx-exec/hx-exec.toml)"
            )
        })?;
        let cfg = Config::load(&path)
            .with_context(|| format!("loading config: {}", path.display()))?;
        let alias = cfg.resolve_alias(alias_name)?;
        Resolved::from_alias(&alias)?
    } else {
        if cli.command.is_empty() {
            return Err(anyhow!(
                "no command given. Use `-c <alias>` or pass a command after `--`."
            ));
        }
        Resolved::from_argv(&cli.command)?
    };

    // If alias was used and extra args were provided, append them.
    if cli.alias.is_some() && !cli.command.is_empty() {
        resolved.push_extra_args(&cli.command)?;
    }

    if cli.print {
        println!("{}", resolved.display());
        return Ok(0);
    }

    resolved.exec()
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("hx-exec: {:#}", e);
            ExitCode::from(1)
        }
    }
}
