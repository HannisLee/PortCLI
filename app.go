package main

import (
	"context"
	"fmt"
	"time"

	"github.com/HannisLee/PortHannis/config"
	"github.com/HannisLee/PortHannis/forwarder"
	"github.com/HannisLee/PortHannis/webui"
	"github.com/google/uuid"
)

type App struct {
	ctx      context.Context
	quitting bool
	mgr      *config.Manager
	engine   *forwarder.Engine
	webui    *webui.Server
}

func NewApp() *App {
	mgr := config.NewManager()
	return &App{
		mgr:    mgr,
		engine: forwarder.NewEngine(mgr),
	}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	if err := a.mgr.LoadConfig(); err != nil {
		fmt.Printf("failed to load config: %v\n", err)
		return
	}
	for _, rule := range a.mgr.GetConfig().Rules {
		if rule.Enabled {
			if err := a.engine.StartRule(rule.ID); err != nil {
				fmt.Printf("failed to start rule %s: %v\n", rule.ID, err)
			}
		}
	}

	// 启动 WebUI
	a.startWebUI()
}

func (a *App) shutdown(ctx context.Context) {
	if a.webui != nil {
		a.webui.Stop(ctx)
	}
	a.engine.StopAll()
}

// Rule management bindings

func (a *App) GetRules() []config.ForwardRule {
	return a.mgr.GetConfig().Rules
}

func (a *App) AddRule(sourceHost string, localPort int, targetHost string, targetPort int) error {
	if localPort < 1024 || localPort > 65535 {
		return fmt.Errorf("local port must be between 1024 and 65535")
	}
	if targetPort < 1 || targetPort > 65535 {
		return fmt.Errorf("target port must be between 1 and 65535")
	}
	if targetHost == "" {
		return fmt.Errorf("target host cannot be empty")
	}
	if sourceHost == "" {
		sourceHost = "0.0.0.0"
	}

	rule := config.ForwardRule{
		ID:         uuid.New().String(),
		SourceHost: sourceHost,
		LocalPort:  localPort,
		TargetHost: targetHost,
		TargetPort: targetPort,
		Enabled:    true,
		CreatedAt:  time.Now(),
	}

	if err := a.mgr.AddRule(rule); err != nil {
		return err
	}
	return a.engine.StartRule(rule.ID)
}

func (a *App) DeleteRule(id string) error {
	a.engine.StopRule(id)
	return a.mgr.DeleteRule(id)
}

func (a *App) ToggleRule(id string, enabled bool) error {
	if err := a.mgr.UpdateRule(id, enabled); err != nil {
		return err
	}
	if enabled {
		return a.engine.StartRule(id)
	}
	return a.engine.StopRule(id)
}

// Log and status bindings

func (a *App) GetLogs(ruleID string, limit int) []config.LogEntry {
	logs, err := a.engine.GetLogs(ruleID, limit)
	if err != nil {
		return []config.LogEntry{}
	}
	return logs
}

func (a *App) ClearLogs(ruleID string) error {
	return a.engine.ClearLogs(ruleID)
}

func (a *App) GetStatus() map[string]bool {
	return a.engine.GetStatus()
}

// WebUI config bindings

func (a *App) GetWebUIConfig() config.WebUIConfig {
	return a.mgr.GetWebUIConfig()
}

func (a *App) UpdateWebUIConfig(enabled bool, port int, password string) error {
	cfg := config.WebUIConfig{
		Enabled:  enabled,
		Port:     port,
		Password: password,
	}
	return a.mgr.UpdateWebUIConfig(cfg)
}

// startWebUI 启动 WebUI 服务器
func (a *App) startWebUI() {
	webuiCfg := a.mgr.GetWebUIConfig()
	if !webuiCfg.Enabled {
		return
	}

	bindings := &webui.AppBindings{
		GetRules:   a.GetRules,
		AddRule:    a.AddRule,
		DeleteRule: a.DeleteRule,
		ToggleRule: a.ToggleRule,
		GetLogs:    a.GetLogs,
		ClearLogs:  a.ClearLogs,
		GetStatus:  a.GetStatus,
		GetWebUI:   a.GetWebUIConfig,
		SetWebUI: func(cfg config.WebUIConfig) error {
			return a.mgr.UpdateWebUIConfig(cfg)
		},
	}

	a.webui = webui.NewServer(bindings, webuiCfg.Port)
	a.webui.Start()
}
