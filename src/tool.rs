use anyhow::Error;
use rayon::prelude::*;
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::path::Path;
use crate::types::FileDiff;

const MAX_OUTPUT_LINES: usize = 500;
const MAX_OUTPUT_CONTEXT: usize = 50;
const MAX_LINE_WIDTH: usize = 2000;

/// Output from tool execution: content for LLM and optional diff for display
#[derive(Debug)]
pub struct ToolOutput {
    pub content: String,
    pub diff: Option<FileDiff>,
}

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

pub fn execute(tool: &ToolCall) -> anyhow::Result<ToolOutput> {
    match tool.name.as_str() {
        "Read" => read(&tool.args),
        "Glob" => glob(&tool.args),
        "Grep" => grep(&tool.args),
        "Bash" => bash(&tool.args),
        "Write" => write_file(&tool.args),
        "Edit" => edit(&tool.args),
        _ => Err(Error::msg(format!("Unknown tool: {}", tool.name))),
    }
}

/// Format a tool call for display (e.g., "Read src/main.rs" instead of full JSON args)
pub fn format_tool_call(tool_name: &str, args: &serde_json::Map<String, Value>) -> String {
    let params = match tool_name {
        "Read" => {
            let path = args.get("filePath").and_then(|v| v.as_str()).unwrap_or("?");
            if let Some(offset) = args.get("offset").and_then(|v| v.as_u64()) {
                format!("{} (offset: {})", path, offset)
            } else {
                path.to_string()
            }
        }
        "Glob" => {
            args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?").to_string()
        }
        "Grep" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let files = args.get("files").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{} in {}", pattern, files)
        }
        "Bash" => {
            args.get("command").and_then(|v| v.as_str()).unwrap_or("?").to_string()
        }
        "Write" => {
            args.get("path").and_then(|v| v.as_str()).unwrap_or("?").to_string()
        }
        "Edit" => {
            args.get("path").and_then(|v| v.as_str()).unwrap_or("?").to_string()
        }
        _ => return tool_name.to_string(),
    };
    format!("{} {}", tool_name, params)
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

fn read(args: &serde_json::Map<String, Value>) -> anyhow::Result<ToolOutput> {
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

    Ok(ToolOutput {
        content: format_truncated(lines_to_show),
        diff: None,
    })
}

fn glob(args: &serde_json::Map<String, Value>) -> anyhow::Result<ToolOutput> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'pattern'"))?;

    let mut matches: Vec<String> = globwalk::glob(pattern)
        .map_err(|e| Error::msg(format!("Invalid glob pattern: {}", e)))?
        .filter_map(|entry| {
            entry.ok().and_then(|dir_entry| {
                let path = dir_entry.path();
                path.to_str()
                    .map(|s| s.trim_start_matches("./").to_string())
            })
        })
        .collect();

    if matches.is_empty() {
        return Ok(ToolOutput {
            content: format!("No files match pattern: {}", pattern),
            diff: None,
        });
    }

    matches.sort();
    let result_lines: Vec<&str> = matches.iter().map(|s| s.as_str()).collect();
    Ok(ToolOutput {
        content: format_truncated(result_lines),
        diff: None,
    })
}

fn grep(args: &serde_json::Map<String, Value>) -> anyhow::Result<ToolOutput> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'pattern'"))?;

    let files_pattern = args
        .get("files")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'files'"))?;

    let regex = Regex::new(pattern)
        .map_err(|e| Error::msg(format!("Invalid regex: {}", e)))?;

    let file_paths: Vec<_> = globwalk::glob(files_pattern)
        .map_err(|e| Error::msg(format!("Invalid file pattern: {}", e)))?
        .filter_map(|e| e.ok())
        .collect();

    if file_paths.is_empty() {
        return Ok(ToolOutput {
            content: format!("No files match pattern: {}", files_pattern),
            diff: None,
        });
    }

    // Parallel search across files
    let matches: Vec<String> = file_paths
        .par_iter()
        .filter_map(|dir_entry| {
            let file_type = dir_entry.file_type();
            if !file_type.is_file() {
                return None;
            }

            let path = dir_entry.path();
            fs::read_to_string(path).ok().and_then(|content| {
                let path_str = path.to_string_lossy();
                let results: Vec<String> = content
                    .lines()
                    .enumerate()
                    .filter_map(|(line_num, line)| {
                        if regex.is_match(line) {
                            Some(format!("{}:{}: {}", path_str, line_num + 1, line))
                        } else {
                            None
                        }
                    })
                    .collect();
                if results.is_empty() {
                    None
                } else {
                    Some(results)
                }
            })
        })
        .flatten()
        .collect();

    if matches.is_empty() {
        return Ok(ToolOutput {
            content: format!("No matches found for pattern: {}", pattern),
            diff: None,
        });
    }

    let result_lines: Vec<&str> = matches.iter().map(|s| s.as_str()).collect();
    Ok(ToolOutput {
        content: format_truncated(result_lines),
        diff: None,
    })
}

fn validate_bash_command(command: &str) -> anyhow::Result<()> {
    let cmd_lower = command.to_lowercase();

    let blocked = [
        ("dd", "disk write operations (data destruction risk)"),
        ("mkfs", "filesystem formatting (irreversible)"),
        ("reboot", "system reboot (would interrupt session)"),
        ("shutdown", "system shutdown (would interrupt session)"),
        ("rm ", "file deletion (data loss risk)"),
        ("rm\t", "file deletion (data loss risk)"),
        ("mv ", "file move/rename (could overwrite data)"),
        ("truncate", "file truncation (destructive)"),
        ("git push --force", "force git push (overwrites history)"),
        ("git push -f", "force git push (overwrites history)"),
        (" | bash", "pipe to bash (code injection risk)"),
        (" | sh", "pipe to shell (code injection risk)"),
    ];

    for (pattern, reason) in &blocked {
        if is_command_match(&cmd_lower, pattern) {
            return Err(Error::msg(format!("Command blocked for safety: {}", reason)));
        }
    }

    Ok(())
}

fn is_command_match(command: &str, pattern: &str) -> bool {
    if command.starts_with(pattern) {
        return true;
    }

    for operator in &["; ", "| ", "& ", "$( ", "` ", "\t", "\n"] {
        if let Some(pos) = command.find(operator) {
            if command[pos + operator.len()..].starts_with(pattern) {
                return true;
            }
        }
    }

    false
}

fn bash(args: &serde_json::Map<String, Value>) -> anyhow::Result<ToolOutput> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'command'"))?;

    validate_bash_command(command)?;

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| Error::msg(format!("Error executing command: {}", e)))?;

    let result = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            stderr.to_string()
        } else {
            format!("Command exited with status: {}", output.status)
        }
    };

    let lines: Vec<&str> = result.lines().collect();
    Ok(ToolOutput {
        content: format_truncated(lines),
        diff: None,
    })
}

/// Compute a diff between old and new content, returning structured diff hunks with context
fn compute_diff(old: &str, new: &str, path: &str, context: usize) -> crate::types::FileDiff {
    use similar::TextDiff;
    use crate::types::{DiffLine, DiffLineKind, DiffHunk};

    let diff = TextDiff::from_lines(old, new);

    // Build list of all diff lines with their line numbers
    let mut all_lines = Vec::new();
    let mut old_lineno = 1usize;
    let mut new_lineno = 1usize;

    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Equal => {
                all_lines.push((new_lineno, DiffLineKind::Context, change.value().trim_end().to_string()));
                old_lineno += 1;
                new_lineno += 1;
            }
            similar::ChangeTag::Insert => {
                all_lines.push((new_lineno, DiffLineKind::Added, change.value().trim_end().to_string()));
                new_lineno += 1;
            }
            similar::ChangeTag::Delete => {
                all_lines.push((old_lineno, DiffLineKind::Removed, change.value().trim_end().to_string()));
                old_lineno += 1;
            }
        }
    }

    // Find lines that have changes
    let changed_indices: Vec<usize> = all_lines
        .iter()
        .enumerate()
        .filter(|(_, (_, kind, _))| *kind != DiffLineKind::Context)
        .map(|(i, _)| i)
        .collect();

    if changed_indices.is_empty() {
        // No changes
        return crate::types::FileDiff {
            path: path.to_string(),
            hunks: vec![],
        };
    }

    // Mark which lines to include (changed lines ± context)
    let mut include = vec![false; all_lines.len()];
    for &idx in &changed_indices {
        let start = idx.saturating_sub(context);
        let end = (idx + context).min(all_lines.len() - 1);
        for i in start..=end {
            include[i] = true;
        }
    }

    // Build hunks (consecutive groups of included lines)
    let mut hunks = Vec::new();
    let mut current_hunk = Vec::new();
    let mut last_included = None;

    for (i, &should_include) in include.iter().enumerate() {
        if should_include {
            let (lineno, kind, content) = &all_lines[i];

            // Check if we should start a new hunk (gap between included regions)
            if let Some(last) = last_included {
                if i > last + 1 {
                    // Gap detected, finalize current hunk and start new one
                    if !current_hunk.is_empty() {
                        hunks.push(DiffHunk { lines: current_hunk.clone() });
                        current_hunk.clear();
                    }
                }
            }

            current_hunk.push(DiffLine {
                kind: *kind,
                lineno: *lineno,
                content: content.clone(),
            });
            last_included = Some(i);
        }
    }

    // Add final hunk if any
    if !current_hunk.is_empty() {
        hunks.push(DiffHunk { lines: current_hunk });
    }

    crate::types::FileDiff {
        path: path.to_string(),
        hunks,
    }
}

fn write_file(args: &serde_json::Map<String, Value>) -> anyhow::Result<ToolOutput> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'path'"))?;

    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'content'"))?;

    // Create parent directories if needed
    let file_path = Path::new(path);
    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| Error::msg(format!("Error creating directories: {}", e)))?;
        }
    }

    // Read old content for diff
    let old_content = fs::read_to_string(path).unwrap_or_default();

    // Write file
    fs::write(path, content)
        .map_err(|e| Error::msg(format!("Error writing file: {}", e)))?;

    // Generate diff
    let diff = compute_diff(&old_content, content, path, 0);
    let summary = format!("Written {} bytes to {}", content.len(), path);

    Ok(ToolOutput {
        content: summary,
        diff: Some(diff),
    })
}

fn edit(args: &serde_json::Map<String, Value>) -> anyhow::Result<ToolOutput> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'path'"))?;

    let old_string = args
        .get("old_string")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'old_string'"))?;

    let new_string = args
        .get("new_string")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("Missing or invalid 'new_string'"))?;

    // Read file
    let content = fs::read_to_string(path)
        .map_err(|e| Error::msg(format!("Error reading file: {}", e)))?;

    // Validate old_string exists and is unique
    if !content.contains(old_string) {
        return Err(Error::msg("old_string not found in file"));
    }

    let count = content.matches(old_string).count();
    if count > 1 {
        return Err(Error::msg(format!(
            "old_string appears {} times (must be unique)",
            count
        )));
    }

    // Replace
    let new_content = content.replacen(old_string, new_string, 1);

    // Write back
    fs::write(path, &new_content)
        .map_err(|e| Error::msg(format!("Error writing file: {}", e)))?;

    // Generate diff
    let diff = compute_diff(&content, &new_content, path, 2);
    let summary = format!("Edited {}", path);

    Ok(ToolOutput {
        content: summary,
        diff: Some(diff),
    })
}
