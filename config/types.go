package config

import "time"

// ForwardRule 单条端口转发规则
type ForwardRule struct {
	ID         string    `json:"id"`
	SourceHost string    `json:"sourceHost"` // 本地监听地址，空或 "0.0.0.0" 表示全部接口
	LocalPort  int       `json:"localPort"`
	TargetHost string    `json:"targetHost"`
	TargetPort int       `json:"targetPort"`
	Enabled    bool      `json:"enabled"`
	CreatedAt  time.Time `json:"createdAt"`
}

// LogEntry 日志条目
type LogEntry struct {
	Timestamp time.Time `json:"timestamp"`
	Source    string    `json:"source"`
	BytesIn   int64     `json:"bytesIn"`
	BytesOut  int64     `json:"bytesOut"`
	Status    string    `json:"status"`
}

// WebUIConfig WebUI 服务配置
type WebUIConfig struct {
	Enabled  bool   `json:"enabled"`  // 是否启用 WebUI，默认 true
	Port     int    `json:"port"`     // 监听端口，默认 18080
	Password string `json:"password"` // 访问密码，空则无需认证
}

// Config 应用配置
type Config struct {
	Rules  []ForwardRule `json:"rules"`
	WebUI  WebUIConfig   `json:"webui"`
}
