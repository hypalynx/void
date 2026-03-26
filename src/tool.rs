use anyhow::Error;
use rayon::prelude::*;
use regex::Regex;
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
        "Glob" => glob(&tool.args),
        "Grep" => grep(&tool.args),
        "Bash" => bash(&tool.args),
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

fn glob(args: &serde_json::Map<String, Value>) -> anyhow::Result<String> {
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
        return Ok(format!("No files match pattern: {}", pattern));
    }

    matches.sort();
    let result_lines: Vec<&str> = matches.iter().map(|s| s.as_str()).collect();
    Ok(format_truncated(result_lines))
}

fn grep(args: &serde_json::Map<String, Value>) -> anyhow::Result<String> {
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
        return Ok(format!("No files match pattern: {}", files_pattern));
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
        return Ok(format!("No matches found for pattern: {}", pattern));
    }

    let result_lines: Vec<&str> = matches.iter().map(|s| s.as_str()).collect();
    Ok(format_truncated(result_lines))
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

fn bash(args: &serde_json::Map<String, Value>) -> anyhow::Result<String> {
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
    Ok(format_truncated(lines))
}
