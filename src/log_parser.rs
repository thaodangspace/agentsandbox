use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Represents different types of log events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEvent {
    #[serde(rename = "session_start")]
    SessionStart {
        timestamp: DateTime<Utc>,
        container: String,
        command: String,
        term: String,
        tty: String,
        columns: u16,
        lines: u16,
    },
    #[serde(rename = "session_end")]
    SessionEnd {
        timestamp: DateTime<Utc>,
        exit_code: i32,
        duration_secs: i64,
    },
    #[serde(rename = "output")]
    Output {
        timestamp: DateTime<Utc>,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        ansi: Option<String>,
    },
}

/// Session metadata extracted from the script log
#[derive(Debug, Clone)]
pub struct SessionMetadata {
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub command: String,
    pub term: String,
    pub tty: String,
    pub columns: u16,
    pub lines: u16,
    pub exit_code: Option<i32>,
}

/// Parse the script header line
/// Format: Script started on 2025-11-04 16:04:17+00:00 [COMMAND="..." TERM="xterm" TTY="/dev/pts/1" COLUMNS="91" LINES="59"]
fn parse_script_header(line: &str) -> Result<SessionMetadata> {
    // Extract timestamp
    let timestamp_end = line
        .find('[')
        .context("Failed to find '[' in script header")?;
    let timestamp_str = line[18..timestamp_end]
        .trim()
        .replace("+00:00", "Z");

    let start_time = DateTime::parse_from_rfc3339(&timestamp_str)
        .with_context(|| format!("Failed to parse timestamp: {}", timestamp_str))?
        .with_timezone(&Utc);

    // Extract attributes
    let attrs_str = &line[timestamp_end + 1..line.len() - 1]; // Remove [ and ]
    let attrs = parse_attributes(attrs_str)?;

    Ok(SessionMetadata {
        start_time,
        end_time: None,
        command: attrs.get("COMMAND").cloned().unwrap_or_default(),
        term: attrs.get("TERM").cloned().unwrap_or_default(),
        tty: attrs.get("TTY").cloned().unwrap_or_default(),
        columns: attrs
            .get("COLUMNS")
            .and_then(|v| v.parse().ok())
            .unwrap_or(80),
        lines: attrs
            .get("LINES")
            .and_then(|v| v.parse().ok())
            .unwrap_or(24),
        exit_code: None,
    })
}

/// Parse the script footer line
/// Format: Script done on 2025-11-04 16:05:14+00:00 [COMMAND_EXIT_CODE="0"]
fn parse_script_footer(line: &str, metadata: &mut SessionMetadata) -> Result<()> {
    // Extract timestamp
    let timestamp_end = line
        .find('[')
        .context("Failed to find '[' in script footer")?;
    let timestamp_str = line[16..timestamp_end]
        .trim()
        .replace("+00:00", "Z");

    let end_time = DateTime::parse_from_rfc3339(&timestamp_str)
        .with_context(|| format!("Failed to parse timestamp: {}", timestamp_str))?
        .with_timezone(&Utc);

    // Extract exit code
    let attrs_str = &line[timestamp_end + 1..line.len() - 1];
    let attrs = parse_attributes(attrs_str)?;
    let exit_code = attrs
        .get("COMMAND_EXIT_CODE")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    metadata.end_time = Some(end_time);
    metadata.exit_code = Some(exit_code);

    Ok(())
}

/// Parse key="value" attributes from script header/footer
fn parse_attributes(attrs_str: &str) -> Result<HashMap<String, String>> {
    let mut attrs = HashMap::new();
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_value = false;
    let mut in_quotes = false;
    let mut escape_next = false;

    for ch in attrs_str.chars() {
        if escape_next {
            current_value.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_quotes => {
                escape_next = true;
            }
            '=' if !in_quotes && !in_value => {
                in_value = true;
            }
            '"' if in_value => {
                in_quotes = !in_quotes;
            }
            ' ' if !in_quotes && in_value => {
                attrs.insert(current_key.clone(), current_value.clone());
                current_key.clear();
                current_value.clear();
                in_value = false;
            }
            _ => {
                if in_value {
                    current_value.push(ch);
                } else {
                    current_key.push(ch);
                }
            }
        }
    }

    // Add the last attribute
    if !current_key.is_empty() {
        attrs.insert(current_key, current_value);
    }

    Ok(attrs)
}

/// Strip ANSI escape sequences and return plain text
pub fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // ESC character
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we find a letter (CSI terminator)
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch.is_ascii_alphabetic() || next_ch == '~' {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence
                chars.next(); // consume ']'
                // Skip until we find BEL or ESC \
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch == '\x07' {
                        break;
                    }
                    if next_ch == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
        } else if ch == '\r' {
            // Ignore carriage returns
            continue;
        } else if ch >= '\x20' || ch == '\n' || ch == '\t' {
            // Only include printable characters
            result.push(ch);
        }
    }

    result
}

/// Parse a raw script log file and return a vector of log events
pub fn parse_raw_log<P: AsRef<Path>>(path: P) -> Result<Vec<LogEvent>> {
    let file = File::open(path.as_ref())
        .with_context(|| format!("Failed to open log file: {:?}", path.as_ref()))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut metadata: Option<SessionMetadata> = None;
    let mut output_buffer = String::new();
    let mut last_output_time: Option<DateTime<Utc>> = None;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("Failed to read line {}", line_num))?;

        // Check for script header
        if line.starts_with("Script started on") {
            match parse_script_header(&line) {
                Ok(meta) => {
                    let container = extract_container_name(&meta.command);
                    let event = LogEvent::SessionStart {
                        timestamp: meta.start_time,
                        container: container.to_string(),
                        command: meta.command.clone(),
                        term: meta.term.clone(),
                        tty: meta.tty.clone(),
                        columns: meta.columns,
                        lines: meta.lines,
                    };
                    events.push(event);
                    last_output_time = Some(meta.start_time);
                    metadata = Some(meta);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse script header at line {}: {}", line_num, e);
                }
            }
            continue;
        }

        // Check for script footer
        if line.starts_with("Script done on") {
            // Flush any remaining output
            if !output_buffer.is_empty() {
                let text = strip_ansi(&output_buffer);
                if !text.trim().is_empty() {
                    events.push(LogEvent::Output {
                        timestamp: last_output_time.unwrap_or_else(Utc::now),
                        text,
                        ansi: Some(output_buffer.clone()),
                    });
                }
                output_buffer.clear();
            }

            if let Some(ref mut meta) = metadata {
                if let Err(e) = parse_script_footer(&line, meta) {
                    eprintln!("Warning: Failed to parse script footer at line {}: {}", line_num, e);
                }

                let duration_secs = if let Some(end_time) = meta.end_time {
                    (end_time - meta.start_time).num_seconds()
                } else {
                    0
                };

                events.push(LogEvent::SessionEnd {
                    timestamp: meta.end_time.unwrap_or_else(Utc::now),
                    exit_code: meta.exit_code.unwrap_or(0),
                    duration_secs,
                });
            }
            continue;
        }

        // Accumulate output
        output_buffer.push_str(&line);
        output_buffer.push('\n');

        // Flush buffer periodically (every 100 lines or when we see a prompt)
        if output_buffer.lines().count() >= 100 || is_prompt_line(&line) {
            let text = strip_ansi(&output_buffer);
            if !text.trim().is_empty() {
                events.push(LogEvent::Output {
                    timestamp: last_output_time.unwrap_or_else(Utc::now),
                    text,
                    ansi: Some(output_buffer.clone()),
                });
            }
            output_buffer.clear();
        }
    }

    // Flush any remaining output
    if !output_buffer.is_empty() {
        let text = strip_ansi(&output_buffer);
        if !text.trim().is_empty() {
            events.push(LogEvent::Output {
                timestamp: last_output_time.unwrap_or_else(Utc::now),
                text,
                ansi: Some(output_buffer.clone()),
            });
        }
    }

    Ok(events)
}

/// Extract container name from working directory
fn extract_container_name(command: &str) -> &str {
    // Try to extract from command path
    if let Some(idx) = command.rfind('/') {
        let dir = &command[idx + 1..];
        if let Some(end) = dir.find('\'') {
            return &dir[..end];
        }
        return dir;
    }
    "unknown"
}

/// Check if a line looks like a shell prompt
fn is_prompt_line(line: &str) -> bool {
    let stripped = strip_ansi(line);
    stripped.contains("$ ") || stripped.contains("# ") || stripped.contains("> ")
}

/// Write log events to a JSONL file
pub fn write_jsonl<P: AsRef<Path>>(events: &[LogEvent], path: P) -> Result<()> {
    use std::io::Write;

    let file = File::create(path.as_ref())
        .with_context(|| format!("Failed to create JSONL file: {:?}", path.as_ref()))?;
    let mut writer = std::io::BufWriter::new(file);

    for event in events {
        let json = serde_json::to_string(event)
            .context("Failed to serialize log event to JSON")?;
        writeln!(writer, "{}", json)
            .context("Failed to write to JSONL file")?;
    }

    writer.flush().context("Failed to flush JSONL file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[31mRed text\x1b[0m Normal text";
        let output = strip_ansi(input);
        assert_eq!(output, "Red text Normal text");
    }

    #[test]
    fn test_parse_attributes() {
        let input = r#"COMMAND="/bin/bash" TERM="xterm" TTY="/dev/pts/1" COLUMNS="91""#;
        let attrs = parse_attributes(input).unwrap();
        assert_eq!(attrs.get("COMMAND"), Some(&"/bin/bash".to_string()));
        assert_eq!(attrs.get("TERM"), Some(&"xterm".to_string()));
        assert_eq!(attrs.get("TTY"), Some(&"/dev/pts/1".to_string()));
        assert_eq!(attrs.get("COLUMNS"), Some(&"91".to_string()));
    }

    #[test]
    fn test_extract_container_name() {
        let command = "cd '/home/dt/Code/agentsandbox' && export PATH";
        let name = extract_container_name(command);
        assert_eq!(name, "agentsandbox");
    }
}
