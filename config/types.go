package config

import "time"

// Rule describes one named TCP forwarding rule in port.json.
type Rule struct {
	SourceHost string `json:"sourceHost"`
	LocalPort  int    `json:"localPort"`
	TargetHost string `json:"targetHost"`
	TargetPort int    `json:"targetPort"`
	Enabled    bool   `json:"enabled"`
	LogPath    string `json:"logPath,omitempty"`
}

// Config is the top-level port.json shape: rule name -> rule.
type Config map[string]Rule

// LogEntry is one per-connection log record.
type LogEntry struct {
	Timestamp time.Time `json:"timestamp"`
	Source    string    `json:"source"`
	BytesIn   int64     `json:"bytesIn"`
	BytesOut  int64     `json:"bytesOut"`
	Status    string    `json:"status"`
}
