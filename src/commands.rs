use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

use crate::cli::{
    Cli, CommandInput, CommandKind, FileArgs, GotoArgs, OutlineArgs, OutputFormat, PositionArgs,
    UsagesArgs, WorkspaceSymbolArgs,
};
use crate::daemon::{self, DaemonRequest};
use crate::model::Output;
use crate::workspace::resolve_workspace_root;

pub(crate) fn run(cli: Cli) -> Result<Output> {
    match cli.command {
        CommandKind::Doctor => run_doctor(cli.format, cli.workspace),
        CommandKind::Goto(args) => run_goto(cli.format, cli.limit, cli.workspace, args),
        CommandKind::Usages(args) => run_usages(cli.format, cli.limit, cli.workspace, args),
        CommandKind::FindSymbol(args) => {
            run_find_symbol(cli.format, cli.limit, cli.workspace, args)
        }
        CommandKind::Inspect(args) => run_inspect(cli.format, cli.workspace, args),
        CommandKind::Outline(args) => run_outline(cli.format, cli.limit, cli.workspace, args),
        CommandKind::Daemon(args) => daemon::run_daemon_command(cli.format, cli.workspace, args),
    }
}

fn run_doctor(format: OutputFormat, workspace_override: Option<PathBuf>) -> Result<Output> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let workspace_root = resolve_workspace_root(workspace_override.as_deref(), None, &cwd)?;
    let adapter = daemon::adapter_status_with_daemon(&workspace_root)?;

    let payload = json!({
        "tool": "lspyx",
        "version": env!("CARGO_PKG_VERSION"),
        "workspace_root": workspace_root,
        "adapter": adapter,
    });

    if format.is_json() {
        return Ok(Output::Json(payload));
    }

    let adapter = &payload["adapter"];
    let ty_line = if adapter["ty"]["found"].as_bool().unwrap_or(false) {
        format!(
            "ty: found at {}",
            adapter["ty"]["path"].as_str().unwrap_or("<unknown>")
        )
    } else {
        "ty: not found".to_string()
    };
    let daemon_line = if adapter["daemon"]["running"].as_bool().unwrap_or(false) {
        format!(
            "daemon: running (pid {})",
            adapter["daemon"]["pid"]
                .as_u64()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )
    } else {
        "daemon: not running".to_string()
    };

    Ok(Output::Text(format!(
        "lspyx {}\nworkspace: {}\n{}\n{}",
        env!("CARGO_PKG_VERSION"),
        workspace_root.display(),
        ty_line,
        daemon_line
    )))
}

fn run_goto(
    format: OutputFormat,
    limit: Option<usize>,
    workspace_override: Option<PathBuf>,
    args: GotoArgs,
) -> Result<Output> {
    let input = CommandInput::from_position_args(args.position)?;
    let workspace_root = resolve_workspace_for_file(workspace_override, &input.file)?;

    daemon::run_via_daemon(
        &workspace_root,
        DaemonRequest::Goto {
            file: input.file,
            line: input.line,
            column: input.column,
            target: args.kind,
            format,
            limit,
        },
        format,
    )
}

fn run_usages(
    format: OutputFormat,
    limit: Option<usize>,
    workspace_override: Option<PathBuf>,
    args: UsagesArgs,
) -> Result<Output> {
    let input = CommandInput::from_position_args(args.position)?;
    let workspace_root = resolve_workspace_for_file(workspace_override, &input.file)?;

    daemon::run_via_daemon(
        &workspace_root,
        DaemonRequest::Usages {
            file: input.file,
            line: input.line,
            column: input.column,
            include_declaration: !args.no_declaration,
            format,
            limit,
        },
        format,
    )
}

fn run_find_symbol(
    format: OutputFormat,
    limit: Option<usize>,
    workspace_override: Option<PathBuf>,
    args: WorkspaceSymbolArgs,
) -> Result<Output> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let workspace_root = resolve_workspace_root(workspace_override.as_deref(), None, &cwd)?;

    daemon::run_via_daemon(
        &workspace_root,
        DaemonRequest::FindSymbol {
            query: args.query,
            kind: args.kind,
            format,
            limit,
        },
        format,
    )
}

fn run_inspect(
    format: OutputFormat,
    workspace_override: Option<PathBuf>,
    args: PositionArgs,
) -> Result<Output> {
    let input = CommandInput::from_position_args(args)?;
    let workspace_root = resolve_workspace_for_file(workspace_override, &input.file)?;

    daemon::run_via_daemon(
        &workspace_root,
        DaemonRequest::Inspect {
            file: input.file,
            line: input.line,
            column: input.column,
            format,
        },
        format,
    )
}

fn run_outline(
    format: OutputFormat,
    limit: Option<usize>,
    workspace_override: Option<PathBuf>,
    args: OutlineArgs,
) -> Result<Output> {
    if args.full && args.depth.is_some() {
        anyhow::bail!("--depth cannot be combined with --full");
    }

    let input = CommandInput::from_file_args(FileArgs { file: args.file })?;
    let workspace_root = resolve_workspace_for_file(workspace_override, &input.file)?;
    let depth = if args.full {
        None
    } else {
        Some(args.depth.unwrap_or(2))
    };

    daemon::run_via_daemon(
        &workspace_root,
        DaemonRequest::Outline {
            file: input.file,
            depth,
            format,
            limit,
        },
        format,
    )
}

fn resolve_workspace_for_file(
    workspace_override: Option<PathBuf>,
    file: &std::path::Path,
) -> Result<PathBuf> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    resolve_workspace_root(workspace_override.as_deref(), Some(file), &cwd)
}
