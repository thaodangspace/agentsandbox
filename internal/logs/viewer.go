package logs

import (
	"fmt"
	"html/template"
	"os"
)

const htmlTemplate = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{.Title}}</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f5f5f5;
        }
        h1 {
            color: #333;
            border-bottom: 2px solid #007bff;
            padding-bottom: 10px;
        }
        .log-entry {
            background: white;
            border-left: 4px solid #007bff;
            margin: 10px 0;
            padding: 15px;
            border-radius: 4px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .log-entry.error {
            border-left-color: #dc3545;
        }
        .log-entry.warning {
            border-left-color: #ffc107;
        }
        .log-entry.info {
            border-left-color: #17a2b8;
        }
        .timestamp {
            color: #666;
            font-size: 0.9em;
            font-family: monospace;
        }
        .level {
            display: inline-block;
            padding: 2px 8px;
            border-radius: 3px;
            font-size: 0.85em;
            font-weight: bold;
            margin-left: 10px;
        }
        .level.error {
            background-color: #dc3545;
            color: white;
        }
        .level.warning {
            background-color: #ffc107;
            color: #333;
        }
        .level.info {
            background-color: #17a2b8;
            color: white;
        }
        .message {
            margin-top: 10px;
            font-size: 1em;
            line-height: 1.5;
        }
        .data {
            margin-top: 10px;
            padding: 10px;
            background-color: #f8f9fa;
            border-radius: 4px;
            font-family: monospace;
            font-size: 0.9em;
        }
    </style>
</head>
<body>
    <h1>{{.Title}}</h1>
    <p>Total events: {{len .Events}}</p>
    
    {{range .Events}}
    <div class="log-entry {{.Level}}">
        <div>
            <span class="timestamp">{{.Timestamp}}</span>
            <span class="level {{.Level}}">{{.Level}}</span>
        </div>
        <div class="message">{{.Message}}</div>
        {{if .Data}}
        <div class="data">{{printf "%+v" .Data}}</div>
        {{end}}
    </div>
    {{end}}
</body>
</html>`

// WriteHTML generates an HTML file from log events
func WriteHTML(events []LogEvent, outputPath string, title string) error {
	tmpl, err := template.New("log").Parse(htmlTemplate)
	if err != nil {
		return fmt.Errorf("failed to parse template: %w", err)
	}

	file, err := os.Create(outputPath)
	if err != nil {
		return fmt.Errorf("failed to create output file: %w", err)
	}
	defer file.Close()

	data := struct {
		Title  string
		Events []LogEvent
	}{
		Title:  title,
		Events: events,
	}

	if err := tmpl.Execute(file, data); err != nil {
		return fmt.Errorf("failed to execute template: %w", err)
	}

	return nil
}

