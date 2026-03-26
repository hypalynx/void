use anyhow::Error;
use serde_json::Value;
use std::fs;
use std::path::Path;

const MAX_OUTPUT_LINES: usize = 500;
const MAX_OUTPUT_CONTEXT: usize = 50;
const MAX_LINE_WIDTH: usize = 2000;

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Map<String, Value>,
}

pub fn definitions() -> Vec<Value> {
    let json_str = include_str!("tool_definitions.json");
    serde_json::from_str(json_str).expect("Failed to parse tool_definitions.json")
}

pub fn execute(tool: &ToolCall) -> anyhow::Result<String> {
    match tool.name.as_str() {
        "Read" => read(&tool.args),
        _ => Err(Error::msg(format!("Unknown tool: {}", tool.name))),
    }
}

/// Resolve a path to an absolute path, handling both absolute and relative paths
fn resolve_path(path: &str) -> anyhow::Result<String> {
    if Path::new(path).is_absolute() {
        Ok(path.to_string())
    } else {
        let cwd = std::env::current_dir()?;
        let full = cwd.join(path);
        full.to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::msg("Invalid path"))
    }
}

/// Format lines with truncation: shows head lines, skipped count, then tail lines
fn format_truncated(lines: Vec<&str>) -> String {
    if lines.len() > MAX_OUTPUT_LINES {
        let head_count = MAX_OUTPUT_LINES - MAX_OUTPUT_CONTEXT;

        let head = lines
            .iter()
            .take(head_count)
            .map(|l| truncate_line(l))
            .collect::<Vec<_>>()
            .join("\n");

        let tail = lines
            .iter()
            .skip(lines.len() - MAX_OUTPUT_CONTEXT)
            .map(|l| truncate_line(l))
            .collect::<Vec<_>>()
            .join("\n");

        let skipped = lines.len() - head_count - MAX_OUTPUT_CONTEXT;
        format!(
            "{}\n\n[... {} lines truncated ...]\n\n{}",
            head, skipped, tail
        )
    } else {
        lines
            .iter()
            .map(|l| truncate_line(l))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Truncate a single line to MAX_LINE_WIDTH
fn truncate_line(line: &str) -> String {
    if line.len() > MAX_LINE_WIDTH {
        format!("{}...", &line[..MAX_LINE_WIDTH])
    } else {
        line.to_string()
    }
}

fn read(args: &serde_json::Map<String, Value>) -> anyhow::Result<String> {
    let path = args
        .get("filePath")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'filePath'"))?;

    let offset = args
        .get("offset")
        .and_then(|v| v.as_u64())
        .map(|v| (v as usize).saturating_sub(1))
        .unwrap_or(0);

    let full_path = resolve_path(path)?;
    let content = fs::read_to_string(&full_path)
        .map_err(|e| Error::msg(format!("Error reading file: {}", e)))?;

    let all_lines: Vec<&str> = content.lines().collect();
    let lines_to_show: Vec<&str> = all_lines.iter().skip(offset).cloned().collect();

    Ok(format_truncated(lines_to_show))
}
