package forwarder

import (
	"fmt"
	"sync"

	"github.com/HannisLee/PortHannis/config"
)

type Engine struct {
	mu      sync.RWMutex
	rules   map[string]*RuleState
	loggers map[string]*CircularLogger
}

func NewEngine() *Engine {
	return &Engine{
		rules:   make(map[string]*RuleState),
		loggers: make(map[string]*CircularLogger),
	}
}

func (e *Engine) StartRule(name string, rule config.Rule) error {
	e.mu.Lock()
	defer e.mu.Unlock()

	if _, ok := e.rules[name]; ok {
		return fmt.Errorf("rule %q is already running", name)
	}
	if rule.LogPath == "" {
		return fmt.Errorf("rule %q has empty logPath", name)
	}

	logger, err := NewCircularLogger(rule.LogPath)
	if err != nil {
		return fmt.Errorf("failed to create logger for %q: %w", name, err)
	}

	state := NewRuleState(name, rule, logger)
	if err := state.Start(); err != nil {
		_ = logger.Close()
		return err
	}

	e.rules[name] = state
	e.loggers[name] = logger
	return nil
}

func (e *Engine) StopRule(name string) error {
	e.mu.Lock()
	defer e.mu.Unlock()

	state, ok := e.rules[name]
	if !ok {
		return fmt.Errorf("rule %q is not running", name)
	}

	state.Stop()
	delete(e.rules, name)

	if logger, ok := e.loggers[name]; ok {
		_ = logger.Close()
		delete(e.loggers, name)
	}
	return nil
}

func (e *Engine) GetStatus() map[string]bool {
	e.mu.RLock()
	defer e.mu.RUnlock()

	status := make(map[string]bool, len(e.rules))
	for name, state := range e.rules {
		status[name] = state.Running()
	}
	return status
}

func (e *Engine) StopAll() {
	e.mu.Lock()
	defer e.mu.Unlock()

	for name, state := range e.rules {
		state.Stop()
		delete(e.rules, name)
	}
	for name, logger := range e.loggers {
		_ = logger.Close()
		delete(e.loggers, name)
	}
}
