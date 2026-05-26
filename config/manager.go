package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strconv"
	"sync"
)

const (
	appName    = "porthannis"
	configFile = "port.json"
	stateFile  = "daemon.json"
)

var safeLogNameRE = regexp.MustCompile(`[^A-Za-z0-9._-]+`)

type Manager struct {
	mu     sync.Mutex
	config Config
	dir    string
}

func NewManager() *Manager {
	return &Manager{
		config: Config{},
		dir:    GetConfigDir(),
	}
}

func GetConfigDir() string {
	switch runtime.GOOS {
	case "windows":
		if appData := os.Getenv("APPDATA"); appData != "" {
			return filepath.Join(appData, appName)
		}
	case "darwin":
		if home, err := os.UserHomeDir(); err == nil {
			return filepath.Join(home, "Library", "Application Support", appName)
		}
	default:
		if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
			return filepath.Join(xdg, appName)
		}
	}

	home, err := os.UserHomeDir()
	if err != nil {
		home = "."
	}
	return filepath.Join(home, ".config", appName)
}

func GetLogsDir() string {
	return filepath.Join(GetConfigDir(), "logs")
}

func GetStatePath() string {
	return filepath.Join(GetConfigDir(), stateFile)
}

func (m *Manager) ConfigPath() string {
	return filepath.Join(m.dir, configFile)
}

func (m *Manager) LogsDir() string {
	return filepath.Join(m.dir, "logs")
}

func (m *Manager) StatePath() string {
	return filepath.Join(m.dir, stateFile)
}

func (m *Manager) LoadConfig() error {
	m.mu.Lock()
	defer m.mu.Unlock()

	data, err := os.ReadFile(m.ConfigPath())
	if err != nil {
		if os.IsNotExist(err) {
			m.config = Config{}
			return nil
		}
		return err
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return err
	}
	if cfg == nil {
		cfg = Config{}
	}
	m.config = cfg
	return nil
}

func (m *Manager) SaveConfig() error {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.saveLocked()
}

func (m *Manager) saveLocked() error {
	if err := os.MkdirAll(m.dir, 0755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(m.config, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return os.WriteFile(m.ConfigPath(), data, 0644)
}

func (m *Manager) Rules() Config {
	m.mu.Lock()
	defer m.mu.Unlock()

	out := make(Config, len(m.config))
	for name, rule := range m.config {
		out[name] = rule
	}
	return out
}

func (m *Manager) Names() []string {
	m.mu.Lock()
	defer m.mu.Unlock()
	return sortedNames(m.config)
}

func (m *Manager) GetRule(name string) (Rule, bool) {
	m.mu.Lock()
	defer m.mu.Unlock()
	rule, ok := m.config[name]
	return rule, ok
}

func (m *Manager) AddRule(name string, rule Rule) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if name == "" {
		name = nextNameLocked(m.config)
	}
	if _, exists := m.config[name]; exists {
		return fmt.Errorf("rule %q already exists", name)
	}
	if rule.LogPath == "" {
		rule.LogPath = m.DefaultLogPath(name)
	}
	m.config[name] = rule
	return m.saveLocked()
}

func (m *Manager) DeleteRule(name string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, exists := m.config[name]; !exists {
		return fmt.Errorf("rule %q not found", name)
	}
	delete(m.config, name)
	return m.saveLocked()
}

func (m *Manager) SetEnabled(name string, enabled bool) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	rule, exists := m.config[name]
	if !exists {
		return fmt.Errorf("rule %q not found", name)
	}
	rule.Enabled = enabled
	if enabled && rule.LogPath == "" {
		rule.LogPath = m.DefaultLogPath(name)
	}
	m.config[name] = rule
	return m.saveLocked()
}

func (m *Manager) DefaultLogPath(name string) string {
	safeName := safeLogNameRE.ReplaceAllString(name, "_")
	if safeName == "" {
		safeName = "rule"
	}
	return filepath.Join(m.LogsDir(), safeName+".log")
}

func (m *Manager) EnsureLogPaths() (bool, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	changed := false
	for name, rule := range m.config {
		if rule.Enabled && rule.LogPath == "" {
			rule.LogPath = m.DefaultLogPath(name)
			m.config[name] = rule
			changed = true
		}
	}
	if !changed {
		return false, nil
	}
	return true, m.saveLocked()
}

func sortedNames(cfg Config) []string {
	names := make([]string, 0, len(cfg))
	for name := range cfg {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func nextNameLocked(cfg Config) string {
	for i := 1; ; i++ {
		name := "name" + strconv.Itoa(i)
		if _, exists := cfg[name]; !exists {
			return name
		}
	}
}
