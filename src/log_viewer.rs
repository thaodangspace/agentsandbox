use crate::log_parser::LogEvent;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Convert ANSI color codes to HTML/CSS
pub fn ansi_to_html(text: &str) -> String {
    ansi_to_html::convert(text).unwrap_or_else(|_| text.to_string())
}

/// Generate HTML content for a session log
pub fn generate_html(events: &[LogEvent], title: &str) -> String {
    let mut html = HTML_HEADER.replace("{{TITLE}}", title);

    for event in events {
        match event {
            LogEvent::SessionStart {
                timestamp,
                container,
                command,
                term,
                tty,
                columns,
                lines,
            } => {
                html.push_str(&format!(
                    r#"<div class="event session-start">
                        <div class="timestamp">{}</div>
                        <div class="content">
                            <h2>Session Started</h2>
                            <table class="metadata">
                                <tr><th>Container:</th><td>{}</td></tr>
                                <tr><th>Command:</th><td><code>{}</code></td></tr>
                                <tr><th>Terminal:</th><td>{} ({}x{}) {}</td></tr>
                            </table>
                        </div>
                    </div>
                    "#,
                    timestamp.format("%Y-%m-%d %H:%M:%S"),
                    escape_html(container),
                    escape_html(command),
                    escape_html(term),
                    columns,
                    lines,
                    escape_html(tty),
                ));
            }
            LogEvent::SessionEnd {
                timestamp,
                exit_code,
                duration_secs,
            } => {
                let duration = format_duration(*duration_secs);
                let status_class = if *exit_code == 0 {
                    "success"
                } else {
                    "error"
                };

                html.push_str(&format!(
                    r#"<div class="event session-end {}">
                        <div class="timestamp">{}</div>
                        <div class="content">
                            <h2>Session Ended</h2>
                            <table class="metadata">
                                <tr><th>Exit Code:</th><td>{}</td></tr>
                                <tr><th>Duration:</th><td>{}</td></tr>
                            </table>
                        </div>
                    </div>
                    "#,
                    status_class,
                    timestamp.format("%Y-%m-%d %H:%M:%S"),
                    exit_code,
                    duration,
                ));
            }
            LogEvent::Output { timestamp, text, ansi } => {
                let content = if let Some(ansi_text) = ansi {
                    ansi_to_html(ansi_text)
                } else {
                    format!("<pre>{}</pre>", escape_html(text))
                };

                html.push_str(&format!(
                    r#"<div class="event output">
                        <div class="timestamp">{}</div>
                        <div class="content">
                            <div class="output-content">{}</div>
                        </div>
                    </div>
                    "#,
                    timestamp.format("%Y-%m-%d %H:%M:%S"),
                    content,
                ));
            }
        }
    }

    html.push_str(HTML_FOOTER);
    html
}

/// Write HTML to a file
pub fn write_html<P: AsRef<Path>>(events: &[LogEvent], path: P, title: &str) -> Result<()> {
    let html = generate_html(events, title);
    let mut file = File::create(path.as_ref())
        .with_context(|| format!("Failed to create HTML file: {:?}", path.as_ref()))?;
    file.write_all(html.as_bytes())
        .context("Failed to write HTML content")?;
    Ok(())
}

/// Escape HTML special characters
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Format duration in seconds to human-readable format
fn format_duration(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

const HTML_HEADER: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{TITLE}}</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Fira Code', 'Droid Sans Mono', 'Source Code Pro', monospace;
            background: #1e1e1e;
            color: #d4d4d4;
            padding: 20px;
            line-height: 1.6;
        }

        .container {
            max-width: 1400px;
            margin: 0 auto;
        }

        h1 {
            color: #569cd6;
            margin-bottom: 20px;
            padding-bottom: 10px;
            border-bottom: 2px solid #3a3a3a;
        }

        .event {
            display: flex;
            margin-bottom: 15px;
            background: #252526;
            border-left: 3px solid #3a3a3a;
            border-radius: 4px;
            overflow: hidden;
        }

        .event.session-start {
            border-left-color: #4ec9b0;
            background: #1a2f2a;
        }

        .event.session-end {
            border-left-color: #569cd6;
            background: #1a2435;
        }

        .event.session-end.error {
            border-left-color: #f48771;
            background: #3a1f1f;
        }

        .timestamp {
            background: #1e1e1e;
            padding: 15px;
            color: #858585;
            font-size: 12px;
            white-space: nowrap;
            min-width: 180px;
            display: flex;
            align-items: center;
        }

        .content {
            padding: 15px;
            flex: 1;
            overflow-x: auto;
        }

        .content h2 {
            color: #4ec9b0;
            font-size: 14px;
            margin-bottom: 10px;
            text-transform: uppercase;
            letter-spacing: 1px;
        }

        .metadata {
            border-collapse: collapse;
        }

        .metadata th {
            text-align: left;
            padding: 5px 15px 5px 0;
            color: #9cdcfe;
            font-weight: normal;
        }

        .metadata td {
            padding: 5px 0;
            color: #d4d4d4;
        }

        .metadata code {
            background: #1e1e1e;
            padding: 2px 6px;
            border-radius: 3px;
            font-size: 12px;
        }

        .output-content {
            font-size: 13px;
            line-height: 1.5;
            white-space: pre-wrap;
            word-break: break-word;
        }

        .output-content pre {
            margin: 0;
            font-family: inherit;
        }

        /* Search and filter controls */
        .controls {
            margin-bottom: 20px;
            padding: 15px;
            background: #252526;
            border-radius: 4px;
            display: flex;
            gap: 10px;
            align-items: center;
        }

        .controls input[type="text"] {
            flex: 1;
            background: #3c3c3c;
            border: 1px solid #555;
            color: #d4d4d4;
            padding: 8px 12px;
            border-radius: 4px;
            font-family: inherit;
            font-size: 13px;
        }

        .controls input[type="text"]:focus {
            outline: none;
            border-color: #007acc;
        }

        .controls button {
            background: #0e639c;
            color: white;
            border: none;
            padding: 8px 16px;
            border-radius: 4px;
            cursor: pointer;
            font-family: inherit;
            font-size: 13px;
        }

        .controls button:hover {
            background: #1177bb;
        }

        .hidden {
            display: none !important;
        }

        /* Collapsible sections */
        .collapse-toggle {
            cursor: pointer;
            user-select: none;
            display: inline-block;
            margin-right: 8px;
        }

        .collapse-toggle::before {
            content: '▼ ';
            display: inline-block;
            transition: transform 0.2s;
        }

        .collapsed .collapse-toggle::before {
            transform: rotate(-90deg);
        }

        .collapsed .output-content {
            display: none;
        }

        /* Scrollbar styling */
        ::-webkit-scrollbar {
            width: 10px;
            height: 10px;
        }

        ::-webkit-scrollbar-track {
            background: #1e1e1e;
        }

        ::-webkit-scrollbar-thumb {
            background: #3a3a3a;
            border-radius: 5px;
        }

        ::-webkit-scrollbar-thumb:hover {
            background: #4a4a4a;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>{{TITLE}}</h1>
        <div class="controls">
            <input type="text" id="search" placeholder="Search logs...">
            <button onclick="searchLogs()">Search</button>
            <button onclick="clearSearch()">Clear</button>
            <button onclick="toggleAll()">Collapse/Expand All</button>
        </div>
        <div id="events">
"#;

const HTML_FOOTER: &str = r#"
        </div>
    </div>
    <script>
        let allCollapsed = false;

        function searchLogs() {
            const query = document.getElementById('search').value.toLowerCase();
            const events = document.querySelectorAll('.event');

            events.forEach(event => {
                const content = event.textContent.toLowerCase();
                if (content.includes(query) || query === '') {
                    event.classList.remove('hidden');
                } else {
                    event.classList.add('hidden');
                }
            });
        }

        function clearSearch() {
            document.getElementById('search').value = '';
            searchLogs();
        }

        function toggleAll() {
            const events = document.querySelectorAll('.event.output');
            allCollapsed = !allCollapsed;

            events.forEach(event => {
                if (allCollapsed) {
                    event.classList.add('collapsed');
                } else {
                    event.classList.remove('collapsed');
                }
            });
        }

        // Add click handlers for collapsible sections
        document.querySelectorAll('.event.output .content').forEach(content => {
            const toggle = document.createElement('span');
            toggle.className = 'collapse-toggle';
            content.insertBefore(toggle, content.firstChild);

            toggle.addEventListener('click', (e) => {
                e.target.closest('.event').classList.toggle('collapsed');
            });
        });

        // Enable search on Enter key
        document.getElementById('search').addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                searchLogs();
            }
        });
    </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("A & B"), "A &amp; B");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3665), "1h 1m 5s");
    }
}
