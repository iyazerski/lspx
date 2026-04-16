use anyhow::Result;
use std::path::Path;

use crate::cli::{GotoTarget, SymbolKindFilter};
use crate::model::{
    DocumentSymbolNode, LocationOutput, LocationRecord, OutlineOutput, ResolvedPosition,
    SymbolAtOutput, WorkspaceSymbolOutput, WorkspaceSymbolRecord, display_path, symbol_kind_name,
};

pub(crate) fn render_location_output(
    limit: Option<usize>,
    payload: &LocationOutput,
) -> Result<String> {
    let total_items = payload.locations.len();
    let locations = apply_limit(payload.locations.as_slice(), limit);
    let mut lines = vec![format!(
        "summary: {}",
        location_summary(locations.len(), total_items, payload)
    )];
    lines.extend(build_position_header(
        &payload.workspace_root,
        &payload.position,
    ));

    if let Some(target) = payload.target {
        lines.push(format!("target: {}", goto_target_name(target)));
    }

    lines.push(format!(
        "items: {}",
        format_count(locations.len(), total_items, "items")
    ));

    if locations.is_empty() {
        lines.push(format!("result: {}", no_location_result(payload)));
        return Ok(lines.join("\n"));
    }

    lines.push("results:".to_string());

    for (index, location) in locations.iter().enumerate() {
        lines.push(format!(
            "{}. {}",
            index + 1,
            format_location(&payload.workspace_root, location)
        ));

        if let Some(snippet) = location.snippet.as_deref() {
            lines.push(format!("   context: {snippet}"));
        }
    }

    Ok(lines.join("\n"))
}

pub(crate) fn render_workspace_symbol_output(
    limit: Option<usize>,
    payload: &WorkspaceSymbolOutput,
    kind_filter: Option<SymbolKindFilter>,
) -> Result<String> {
    let symbols = payload
        .symbols
        .iter()
        .filter(|symbol| kind_filter.is_none_or(|filter| filter.matches(symbol.kind)))
        .collect::<Vec<_>>();
    let mut symbols = select_workspace_symbols(payload.query.as_str(), symbols);
    let total_items = symbols.len();

    if let Some(n) = limit {
        symbols.truncate(n);
    }

    let mut lines = vec![format!(
        "summary: {}",
        workspace_symbol_summary(symbols.len(), total_items, &payload.query)
    )];
    lines.push(format!("query: {}", payload.query));

    if let Some(kind_filter) = kind_filter {
        lines.push(format!("kind: {}", workspace_kind_name(kind_filter)));
    }

    lines.push(format!(
        "items: {}",
        format_count(symbols.len(), total_items, "symbols")
    ));

    if symbols.is_empty() {
        lines.push(format!(
            "result: no symbols found for query {:?}",
            payload.query
        ));
        return Ok(lines.join("\n"));
    }

    lines.push("results:".to_string());

    for (index, symbol) in symbols.iter().enumerate() {
        lines.push(format!(
            "{}. {} [{}]",
            index + 1,
            symbol.name,
            symbol_kind_name(symbol.kind)
        ));
        lines.push(format!(
            "   location: {}",
            format_workspace_position(
                &payload.workspace_root,
                &symbol.file,
                symbol.range.start.line,
                Some(symbol.range.start.column),
            )
        ));

        if let Some(container) = symbol.container_name.as_deref() {
            lines.push(format!("   container: {container}"));
        }

        if let Some(snippet) = symbol.snippet.as_deref() {
            lines.push(format!("   context: {snippet}"));
        }
    }

    Ok(lines.join("\n"))
}

pub(crate) fn render_symbol_at_output(payload: &SymbolAtOutput) -> Result<String> {
    let mut lines = vec![format!(
        "summary: {}",
        symbol_at_summary(&payload.workspace_root, payload)
    )];
    lines.extend(build_position_header(
        &payload.workspace_root,
        &payload.position,
    ));

    match payload.symbol.as_ref() {
        Some(symbol) => {
            lines.push(format!("result: found symbol {:?}", symbol.name));
            lines.push(format!("symbol: {}", symbol.name));

            if let Some(kind) = symbol.kind {
                lines.push(format!("kind: {}", symbol_kind_name(kind)));
            }

            lines.push(format!(
                "range: columns {}-{}",
                symbol.start_column, symbol.end_column
            ));

            if let Some(detail) = symbol.detail.as_deref() {
                lines.push(format!("detail: {detail}"));
            }

            if let Some(hover) = payload.hover.as_deref()
                && !hover.trim().is_empty()
            {
                lines.push("hover:".to_string());
                lines.extend(indent_block(hover, "  "));
            }
        }
        None => {
            lines.push(format!(
                "result: no symbol found at {}",
                format_requested_position(&payload.workspace_root, &payload.position)
            ));
        }
    }

    Ok(lines.join("\n"))
}

pub(crate) fn render_outline_output(
    limit: Option<usize>,
    payload: &OutlineOutput,
) -> Result<String> {
    let total_items = payload.symbols.len();
    let symbols = apply_limit(payload.symbols.as_slice(), limit);
    let mut lines = vec![
        format!(
            "summary: {}",
            outline_summary(
                symbols.len(),
                total_items,
                &payload.workspace_root,
                &payload.file
            )
        ),
        format!(
            "file: {}",
            display_path(&payload.workspace_root, &payload.file)
        ),
        format!(
            "depth: {}",
            payload
                .depth
                .map(|value| value.to_string())
                .unwrap_or_else(|| "full".to_string())
        ),
        format!(
            "items: {}",
            format_count(symbols.len(), total_items, "top-level symbols")
        ),
    ];

    if symbols.is_empty() {
        lines.push(format!(
            "result: no symbols found in {}",
            display_path(&payload.workspace_root, &payload.file)
        ));
        return Ok(lines.join("\n"));
    }

    lines.push("tree:".to_string());
    for symbol in symbols {
        lines.extend(format_outline_tree(symbol, 0));
    }

    Ok(lines.join("\n"))
}

fn apply_limit<T>(items: &[T], limit: Option<usize>) -> &[T] {
    match limit {
        Some(n) => &items[..n.min(items.len())],
        None => items,
    }
}

fn build_position_header(workspace_root: &Path, position: &ResolvedPosition) -> Vec<String> {
    let mut lines = Vec::new();

    if position.resolved_column.is_some() {
        lines.push(format!(
            "resolved: {}",
            format_resolved_position(workspace_root, position)
        ));
    }

    if let Some(symbol) = position.symbol.as_ref() {
        lines.push(format!("subject: {}", symbol.name));
    }

    if let Some(source_line) = position
        .source_line
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("source: {source_line}"));
    }

    lines
}

fn format_count(shown: usize, total: usize, label: &str) -> String {
    if shown == total {
        total.to_string()
    } else {
        format!("{shown} shown of {total} {label}")
    }
}

fn format_summary_count(shown: usize, total: usize, singular: &str, plural: &str) -> String {
    if shown == total {
        if total == 1 {
            format!("1 {singular}")
        } else {
            format!("{total} {plural}")
        }
    } else {
        format!("{shown} shown of {total} {plural}")
    }
}

fn location_summary(shown: usize, total: usize, payload: &LocationOutput) -> String {
    if shown == 0 {
        return no_location_result(payload);
    }

    match payload.position.symbol.as_ref() {
        Some(symbol) => match payload.target {
            Some(target) => {
                let (singular, plural) = goto_target_labels(target);
                format!(
                    "{} for {}",
                    format_summary_count(shown, total, singular, plural),
                    symbol.name
                )
            }
            None => format!(
                "{} of {}",
                format_summary_count(shown, total, "usage", "usages"),
                symbol.name
            ),
        },
        None => format_summary_count(shown, total, "location", "locations"),
    }
}

fn workspace_symbol_summary(shown: usize, total: usize, query: &str) -> String {
    if shown == 0 {
        return format!("no symbols found for query {:?}", query);
    }

    format!(
        "{} for query {:?}",
        format_summary_count(shown, total, "symbol", "symbols"),
        query
    )
}

fn symbol_at_summary(workspace_root: &Path, payload: &SymbolAtOutput) -> String {
    match payload.symbol.as_ref() {
        Some(symbol) => format!("found symbol {:?}", symbol.name),
        None => format!(
            "no symbol found at {}",
            format_requested_position(workspace_root, &payload.position)
        ),
    }
}

fn outline_summary(shown: usize, total: usize, workspace_root: &Path, file: &Path) -> String {
    let display_file = display_path(workspace_root, file);

    if shown == 0 {
        return format!("no symbols found in {display_file}");
    }

    format!(
        "{} in {}",
        format_summary_count(shown, total, "top-level symbol", "top-level symbols"),
        display_file
    )
}

fn no_location_result(payload: &LocationOutput) -> String {
    let requested = format_requested_position(&payload.workspace_root, &payload.position);

    match payload.position.symbol.as_ref() {
        Some(symbol) => match payload.target {
            Some(target) => format!(
                "no {} found for symbol {:?} at {}",
                goto_target_name(target),
                symbol.name,
                requested
            ),
            None => format!(
                "no usages found for symbol {:?} at {}",
                symbol.name, requested
            ),
        },
        None => format!("no symbol found at {}", requested),
    }
}

fn goto_target_name(target: GotoTarget) -> &'static str {
    match target {
        GotoTarget::Definition => "definition",
        GotoTarget::Declaration => "declaration",
        GotoTarget::Type => "type definition",
    }
}

fn goto_target_labels(target: GotoTarget) -> (&'static str, &'static str) {
    match target {
        GotoTarget::Definition => ("definition", "definitions"),
        GotoTarget::Declaration => ("declaration", "declarations"),
        GotoTarget::Type => ("type definition", "type definitions"),
    }
}

fn workspace_kind_name(kind: SymbolKindFilter) -> &'static str {
    match kind {
        SymbolKindFilter::Class => "class",
        SymbolKindFilter::Function => "function",
        SymbolKindFilter::Method => "method",
    }
}

fn format_requested_position(workspace_root: &Path, position: &ResolvedPosition) -> String {
    format_workspace_position(
        workspace_root,
        &position.file,
        position.line,
        Some(position.requested_column),
    )
}

fn format_resolved_position(workspace_root: &Path, position: &ResolvedPosition) -> String {
    format_workspace_position(
        workspace_root,
        &position.file,
        position.line,
        position.resolved_column,
    )
}

fn format_location(workspace_root: &Path, location: &LocationRecord) -> String {
    format_workspace_position(
        workspace_root,
        &location.file,
        location.range.start.line,
        Some(location.range.start.column),
    )
}

fn format_workspace_position(
    workspace_root: &Path,
    file: &Path,
    line: usize,
    column: Option<usize>,
) -> String {
    let file = display_path(workspace_root, file);

    match column {
        Some(column) => format!("{file}:{line}:{column}"),
        None => format!("{file}:{line}"),
    }
}

fn format_outline_tree(symbol: &DocumentSymbolNode, indent: usize) -> Vec<String> {
    let prefix = "  ".repeat(indent);
    let mut lines = vec![format!(
        "{prefix}- {} [{}] @ {}:{}",
        symbol.name,
        symbol_kind_name(symbol.kind),
        symbol.range.start.line,
        symbol.range.start.column
    )];

    for child in &symbol.children {
        lines.extend(format_outline_tree(child, indent + 1));
    }

    lines
}

fn indent_block(text: &str, prefix: &str) -> Vec<String> {
    text.lines().map(|line| format!("{prefix}{line}")).collect()
}

fn select_workspace_symbols<'a>(
    query: &str,
    symbols: Vec<&'a WorkspaceSymbolRecord>,
) -> Vec<&'a WorkspaceSymbolRecord> {
    let exact_case_sensitive = symbols
        .iter()
        .copied()
        .filter(|symbol| symbol.name == query)
        .collect::<Vec<_>>();
    if !exact_case_sensitive.is_empty() {
        return exact_case_sensitive;
    }

    let exact_case_insensitive = symbols
        .iter()
        .copied()
        .filter(|symbol| symbol.name.eq_ignore_ascii_case(query))
        .collect::<Vec<_>>();
    if !exact_case_insensitive.is_empty() {
        return exact_case_insensitive;
    }

    symbols
}
