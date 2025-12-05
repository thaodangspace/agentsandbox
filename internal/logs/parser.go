package logs

import (
	"bufio"
	"encoding/json"
	"os"
)

// LogEvent represents a single log event from a JSONL file
type LogEvent struct {
	Timestamp string                 `json:"timestamp"`
	Level     string                 `json:"level"`
	Message   string                 `json:"message"`
	Data      map[string]interface{} `json:"data,omitempty"`
}

// ParseRawLog parses a JSONL log file and returns the events
func ParseRawLog(logFile string) ([]LogEvent, error) {
	file, err := os.Open(logFile)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var events []LogEvent
	scanner := bufio.NewScanner(file)

	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			continue
		}

		var event LogEvent
		if err := json.Unmarshal([]byte(line), &event); err != nil {
			// Skip invalid lines
			continue
		}

		events = append(events, event)
	}

	if err := scanner.Err(); err != nil {
		return nil, err
	}

	return events, nil
}

