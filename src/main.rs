use hx_exec::{
    config::{find_config, Config},
    runner::Resolved,
    Result,
};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

struct Cli {
    /// Named alias from hx-exec.toml to execute.
    alias: Option<String>,
    /// Explicit path to an hx-exec.toml file.
    file: Option<PathBuf>,
    /// Print the fully resolved command instead of executing it.
    print: bool,
    /// The command (and args) to run.
    command: Vec<String>,
}

/// Convert an `OsString` command argument to `String`, returning a user-friendly
/// error if it is not valid UTF-8.
fn os_to_string(v: OsString) -> Result<String> {
    v.into_string()
        .map_err(|bad| format!("command argument is not valid UTF-8: {:?}", bad).into())
}

/// Validate that a flag value `raw_val` does not look like another flag
/// (i.e. does not start with `-`).  Returns an error with `flag` in the
/// message if it does.
fn reject_flag_as_value(flag: &str, raw_val: &OsString) -> Result<()> {
    if raw_val.to_str().map_or(false, |s| s.starts_with('-')) {
        return Err(format!(
            "flag `{}` requires a value, got `{}`",
            flag,
            raw_val.to_string_lossy()
        )
        .into());
    }
    Ok(())
}

fn parse_args() -> Result<Cli> {
    let mut cli = Cli {
        alias: None,
        file: None,
        print: false,
        command: Vec::new(),
    };
    let mut args = std::env::args_os().skip(1).peekable();
    while let Some(raw) = args.next() {
        // Reject non-UTF-8 flag tokens (paths are handled separately below)
        let arg = raw.to_str().ok_or("argument is not valid UTF-8")?;
        match arg {
            "-c" | "--config" => {
                let raw_val = args
                    .next()
                    .ok_or_else(|| format!("flag `{}` requires a value", arg))?;
                reject_flag_as_value(arg, &raw_val)?;
                cli.alias = Some(
                    raw_val
                        .into_string()
                        .map_err(|v| format!("value for `{}` is not valid UTF-8: {:?}", arg, v))?,
                );
            }
            "-f" | "--file" => {
                let raw_val = args
                    .next()
                    .ok_or_else(|| format!("flag `{}` requires a value", arg))?;
                reject_flag_as_value(arg, &raw_val)?;
                // PathBuf accepts OsString directly — no UTF-8 conversion needed
                cli.file = Some(PathBuf::from(raw_val));
            }
            "--print" => cli.print = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("hx-exec {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--" => {
                for rest in args {
                    cli.command.push(os_to_string(rest)?);
                }
                break;
            }
            arg if arg.starts_with('-') => {
                return Err(format!("unknown flag `{}`", arg).into());
            }
            _ => {
                cli.command.push(arg.to_owned());
                for rest in args {
                    cli.command.push(os_to_string(rest)?);
                }
                break;
            }
        }
    }
    Ok(cli)
}

fn print_help() {
    println!(
        "hx-exec {}
Expand ${{VAR}} / $(cmd) and launch — built for Helix LSPs across OSes.

USAGE:
    hx-exec [OPTIONS] [-- <CMD>...]
    hx-exec [OPTIONS] <CMD>...

OPTIONS:
    -c, --config <ALIAS>   Named alias from hx-exec.toml to execute
    -f, --file <PATH>      Explicit path to an hx-exec.toml file
        --print            Print the fully resolved command instead of executing it
    -h, --help             Print this help message
    -V, --version          Print version

EXAMPLES:
    hx-exec -- ngserver --stdio --tsProbeLocations \"$(npm -g root)\"
    hx-exec -c angular-lsp
    hx-exec -c angular-lsp -- --extra-flag
    hx-exec --print -- my-lsp --root \"${{HELIX_CONFIG}}\"",
        env!("CARGO_PKG_VERSION")
    );
}

fn run() -> Result<i32> {
    let cli = parse_args()?;

    let mut resolved = if let Some(alias_name) = &cli.alias {
        let path = find_config(cli.file.as_deref()).ok_or(
            "could not locate hx-exec.toml (looked at --file, ./hx-exec.toml, \
             ${HELIX_CONFIG}/hx-exec.toml, and <config>/hx-exec/hx-exec.toml)",
        )?;
        let cfg = Config::load(&path)
            .map_err(|e| format!("loading config {}: {}", path.display(), e))?;
        let alias = cfg.resolve_alias(alias_name)?;
        Resolved::from_alias(alias)?
    } else {
        if cli.command.is_empty() {
            return Err(
                "no command given. Use `-c <alias>` or pass a command after `--`.".into(),
            );
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
            eprintln!("hx-exec: {}", e);
            ExitCode::from(1)
        }
    }
}
