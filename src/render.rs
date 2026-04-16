use anyhow::Result;
use serde_json::{Map, Value, json};
use std::path::Path;

use crate::cli::{OutputFormat, SymbolKindFilter};
use crate::model::{
    DocumentSymbolNode, LocationOutput, LocationRecord, OutlineOutput, Output, RangeRecord,
    SymbolAtOutput, WorkspaceSymbolOutput, WorkspaceSymbolRecord, display_path, symbol_kind_name,
};
use crate::parse::count_document_symbols;

pub(crate) fn render_location_output(
    format: OutputFormat,
    limit: Option<usize>,
    payload: &LocationOutput,
) -> Result<Output> {
    // Count always reflects the full result set, ignoring limit.
    if format.is_count() {
        return Ok(Output::Text(payload.locations.len().to_string()));
    }

    let locations = apply_limit(payload.locations.as_slice(), limit);

    if format.is_paths() {
        let paths = unique_location_paths(&payload.workspace_root, locations);
        return Ok(Output::Text(paths.join("\n")));
    }

    if format.is_json() {
        let results = Value::Array(
            locations
                .iter()
                .map(|location| location_to_value(&payload.workspace_root, location))
                .collect(),
        );
        return Ok(json_envelope(
            &payload.command,
            &payload.workspace_root,
            results,
        ));
    }

    Ok(Output::Text(format_locations_text(
        &payload.workspace_root,
        locations,
    )))
}

pub(crate) fn render_workspace_symbol_output(
    format: OutputFormat,
    limit: Option<usize>,
    payload: &WorkspaceSymbolOutput,
    kind_filter: Option<SymbolKindFilter>,
) -> Result<Output> {
    let symbols = payload
        .symbols
        .iter()
        .filter(|symbol| kind_filter.is_none_or(|filter| filter.matches(symbol.kind)))
        .collect::<Vec<_>>();
    let mut symbols = select_workspace_symbols(payload.query.as_str(), symbols);

    // Count always reflects the full result set, ignoring limit.
    if format.is_count() {
        return Ok(Output::Text(symbols.len().to_string()));
    }

    if let Some(n) = limit {
        symbols.truncate(n);
    }

    if format.is_paths() {
        let paths = unique_workspace_symbol_paths(&payload.workspace_root, &symbols);
        return Ok(Output::Text(paths.join("\n")));
    }

    if format.is_json() {
        let results = Value::Array(
            symbols
                .iter()
                .map(|symbol| workspace_symbol_to_value(&payload.workspace_root, symbol))
                .collect(),
        );
        return Ok(json_envelope(
            &payload.command,
            &payload.workspace_root,
            results,
        ));
    }

    Ok(Output::Text(if symbols.is_empty() {
        "no results".to_string()
    } else {
        symbols
            .iter()
            .map(|symbol| {
                let container = symbol
                    .container_name
                    .as_deref()
                    .map(|name| format!("  ({name})"))
                    .unwrap_or_default();
                let snippet = symbol
                    .snippet
                    .as_deref()
                    .map(|s| format!("\n  {s}"))
                    .unwrap_or_default();
                format!(
                    "{} [{}] {}:{}:{}{}{}",
                    symbol.name,
                    symbol_kind_name(symbol.kind),
                    display_path(&payload.workspace_root, &symbol.file),
                    symbol.range.start.line,
                    symbol.range.start.column,
                    container,
                    snippet
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }))
}

pub(crate) fn render_symbol_at_output(
    format: OutputFormat,
    payload: &SymbolAtOutput,
) -> Result<Output> {
    // Count: 1 if symbol found, 0 if not.
    if format.is_count() {
        let count = if payload.symbol.is_some() { 1 } else { 0 };
        return Ok(Output::Text(count.to_string()));
    }

    // Paths: the file being inspected.
    if format.is_paths() {
        return Ok(Output::Text(display_path(
            &payload.workspace_root,
            &payload.file,
        )));
    }

    if format.is_json() {
        let results = json!({
            "symbol": payload.symbol.as_ref().map(|symbol| {
                json!({
                    "name": symbol.name,
                    "start_column": symbol.start_column,
                    "end_column": symbol.end_column,
                })
            }),
            "hover": payload.hover,
        });
        return Ok(json_envelope(
            &payload.command,
            &payload.workspace_root,
            results,
        ));
    }

    let symbol_text = payload
        .symbol
        .as_ref()
        .map(|symbol| symbol.name.clone())
        .unwrap_or_else(|| "no symbol".to_string());
    let hover_text = payload.hover.as_deref().unwrap_or("");

    Ok(Output::Text(format!(
        "{}:{}:{}\n{}\n{}",
        display_path(&payload.workspace_root, &payload.file),
        payload.line,
        payload.column,
        symbol_text,
        hover_text
    )))
}

pub(crate) fn render_outline_output(
    format: OutputFormat,
    limit: Option<usize>,
    payload: &OutlineOutput,
) -> Result<Output> {
    // Paths: the file being outlined.
    if format.is_paths() {
        return Ok(Output::Text(display_path(
            &payload.workspace_root,
            &payload.file,
        )));
    }

    if format.is_count() {
        return Ok(Output::Text(
            count_document_symbols(payload.symbols.as_slice()).to_string(),
        ));
    }

    let symbols = apply_limit(payload.symbols.as_slice(), limit);

    if format.is_json() {
        let results = Value::Array(
            symbols
                .iter()
                .map(document_symbol_to_compact_value)
                .collect(),
        );
        // Include effective depth in the envelope so agents know whether pruning occurred.
        let mut envelope = json!({
            "ok": true,
            "command": &payload.command,
            "workspace": payload.workspace_root.display().to_string(),
            "results": results,
        });
        if let Some(depth) = payload.depth {
            envelope
                .as_object_mut()
                .unwrap()
                .insert("depth".to_string(), json!(depth));
        }
        return Ok(Output::Json(envelope));
    }

    Ok(Output::Text(
        symbols
            .iter()
            .map(DocumentSymbolNode::format_text)
            .collect::<Vec<_>>()
            .join("\n"),
    ))
}

fn apply_limit<T>(items: &[T], limit: Option<usize>) -> &[T] {
    match limit {
        Some(n) => &items[..n.min(items.len())],
        None => items,
    }
}

fn json_envelope(command: &str, workspace_root: &Path, results: Value) -> Output {
    Output::Json(json!({
        "ok": true,
        "command": command,
        "workspace": workspace_root.display().to_string(),
        "results": results,
    }))
}

fn range_to_value(range: &RangeRecord) -> Value {
    json!({
        "start": {
            "line": range.start.line,
            "column": range.start.column,
        },
        "end": {
            "line": range.end.line,
            "column": range.end.column,
        }
    })
}

fn document_symbol_to_compact_value(symbol: &DocumentSymbolNode) -> Value {
    let mut value = Map::new();
    value.insert("name".to_string(), Value::String(symbol.name.clone()));
    value.insert(
        "kind".to_string(),
        Value::String(symbol_kind_name(symbol.kind).to_string()),
    );
    value.insert("range".to_string(), range_to_value(&symbol.range));
    if let Some(detail) = &symbol.detail {
        value.insert("detail".to_string(), Value::String(detail.clone()));
    }
    value.insert(
        "children".to_string(),
        Value::Array(
            symbol
                .children
                .iter()
                .map(document_symbol_to_compact_value)
                .collect(),
        ),
    );
    Value::Object(value)
}

fn location_to_value(workspace_root: &Path, location: &LocationRecord) -> Value {
    let mut value = Map::new();
    value.insert(
        "file".to_string(),
        Value::String(display_path(workspace_root, &location.file)),
    );
    value.insert("range".to_string(), range_to_value(&location.range));
    Value::Object(value)
}

fn workspace_symbol_to_value(workspace_root: &Path, symbol: &WorkspaceSymbolRecord) -> Value {
    let mut value = Map::new();
    value.insert("name".to_string(), Value::String(symbol.name.clone()));
    value.insert(
        "kind".to_string(),
        Value::String(symbol_kind_name(symbol.kind).to_string()),
    );
    value.insert(
        "file".to_string(),
        Value::String(display_path(workspace_root, &symbol.file)),
    );
    value.insert("range".to_string(), range_to_value(&symbol.range));
    if let Some(container_name) = &symbol.container_name {
        value.insert(
            "container_name".to_string(),
            Value::String(container_name.clone()),
        );
    }
    if let Some(snippet) = &symbol.snippet {
        value.insert("snippet".to_string(), Value::String(snippet.clone()));
    }
    Value::Object(value)
}

fn unique_location_paths(workspace_root: &Path, locations: &[LocationRecord]) -> Vec<String> {
    let mut paths = Vec::new();
    for location in locations {
        let value = display_path(workspace_root, &location.file);
        if !paths.contains(&value) {
            paths.push(value);
        }
    }
    paths
}

fn unique_workspace_symbol_paths(
    workspace_root: &Path,
    symbols: &[&WorkspaceSymbolRecord],
) -> Vec<String> {
    let mut paths = Vec::new();
    for symbol in symbols {
        let value = display_path(workspace_root, &symbol.file);
        if !paths.contains(&value) {
            paths.push(value);
        }
    }
    paths
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

fn format_locations_text(workspace_root: &Path, locations: &[LocationRecord]) -> String {
    if locations.is_empty() {
        return "no results".to_string();
    }

    locations
        .iter()
        .map(|location| {
            let snippet = location
                .snippet
                .as_ref()
                .map_or(String::new(), |value| format!("\n  {value}"));

            format!(
                "{}:{}:{}{}",
                display_path(workspace_root, &location.file),
                location.range.start.line,
                location.range.start.column,
                snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
