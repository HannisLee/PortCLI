package forwarder

import (
	"bufio"
	"encoding/json"
	"os"
	"path/filepath"
	"sync"

	"github.com/HannisLee/PortHannis/config"
)

const defaultMaxSize = 10 * 1024 * 1024 // 10 MB

type CircularLogger struct {
	mu      sync.Mutex
	file    *os.File
	path    string
	maxSize int64
}

func NewCircularLogger(path string) (*CircularLogger, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return nil, err
	}
	f, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR|os.O_APPEND, 0644)
	if err != nil {
		return nil, err
	}
	return &CircularLogger{
		file:    f,
		path:    path,
		maxSize: defaultMaxSize,
	}, nil
}

func (l *CircularLogger) Write(entry config.LogEntry) error {
	data, err := json.Marshal(entry)
	if err != nil {
		return err
	}
	data = append(data, '\n')

	l.mu.Lock()
	defer l.mu.Unlock()

	if info, err := l.file.Stat(); err == nil && info.Size()+int64(len(data)) > l.maxSize {
		if err := l.rotate(); err != nil {
			return err
		}
	}
	_, err = l.file.Write(data)
	return err
}

func (l *CircularLogger) ReadLogs(limit int) ([]config.LogEntry, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	return readLogFile(l.path, limit)
}

func (l *CircularLogger) Clear() error {
	l.mu.Lock()
	defer l.mu.Unlock()

	if err := l.file.Close(); err != nil {
		return err
	}
	f, err := os.Create(l.path)
	if err != nil {
		return err
	}
	l.file = f
	return nil
}

func (l *CircularLogger) Close() error {
	l.mu.Lock()
	defer l.mu.Unlock()
	return l.file.Close()
}

func (l *CircularLogger) rotate() error {
	if err := l.file.Close(); err != nil {
		return err
	}
	f, err := os.Create(l.path)
	if err != nil {
		return err
	}
	l.file = f
	return nil
}

func ReadLogFile(path string, limit int) ([]config.LogEntry, error) {
	return readLogFile(path, limit)
}

func ClearLogFile(path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}
	return os.WriteFile(path, nil, 0644)
}

func readLogFile(path string, limit int) ([]config.LogEntry, error) {
	f, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return []config.LogEntry{}, nil
		}
		return nil, err
	}
	defer f.Close()

	var all []config.LogEntry
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		var entry config.LogEntry
		if err := json.Unmarshal(scanner.Bytes(), &entry); err != nil {
			continue
		}
		all = append(all, entry)
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	if limit <= 0 || limit >= len(all) {
		return all, nil
	}
	return all[len(all)-limit:], nil
}
