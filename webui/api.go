package webui

import (
	"net/http"
	"strconv"

	"github.com/HannisLee/PortHannis/config"
	"github.com/labstack/echo/v4"
)

// registerAPI 注册 REST API 路由
func (s *Server) registerAPI() {
	api := s.echo.Group("/api")

	api.GET("/rules", s.handleGetRules)
	api.POST("/rules", s.handleAddRule)
	api.DELETE("/rules/:id", s.handleDeleteRule)
	api.PUT("/rules/:id/toggle", s.handleToggleRule)
	api.GET("/rules/:id/logs", s.handleGetLogs)
	api.DELETE("/rules/:id/logs", s.handleClearLogs)
	api.GET("/status", s.handleGetStatus)
	api.GET("/webui-config", s.handleGetWebUIConfig)
	api.PUT("/webui-config", s.handleUpdateWebUIConfig)
}

func (s *Server) handleGetRules(c echo.Context) error {
	rules := s.app.GetRules()
	return c.JSON(http.StatusOK, rules)
}

func (s *Server) handleAddRule(c echo.Context) error {
	var body struct {
		SourceHost string `json:"sourceHost"`
		LocalPort  int    `json:"localPort"`
		TargetHost string `json:"targetHost"`
		TargetPort int    `json:"targetPort"`
	}
	if err := c.Bind(&body); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	if err := s.app.AddRule(body.SourceHost, body.LocalPort, body.TargetHost, body.TargetPort); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	return c.JSON(http.StatusCreated, map[string]string{"status": "created"})
}

func (s *Server) handleDeleteRule(c echo.Context) error {
	id := c.Param("id")
	if err := s.app.DeleteRule(id); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}
	return c.JSON(http.StatusOK, map[string]string{"status": "deleted"})
}

func (s *Server) handleToggleRule(c echo.Context) error {
	id := c.Param("id")
	var body struct {
		Enabled bool `json:"enabled"`
	}
	if err := c.Bind(&body); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	if err := s.app.ToggleRule(id, body.Enabled); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}
	return c.JSON(http.StatusOK, map[string]string{"status": "updated"})
}

func (s *Server) handleGetLogs(c echo.Context) error {
	id := c.Param("id")
	limit := 500
	if l := c.QueryParam("limit"); l != "" {
		if v, err := strconv.Atoi(l); err == nil {
			limit = v
		}
	}
	logs := s.app.GetLogs(id, limit)
	return c.JSON(http.StatusOK, logs)
}

func (s *Server) handleClearLogs(c echo.Context) error {
	id := c.Param("id")
	if err := s.app.ClearLogs(id); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}
	return c.JSON(http.StatusOK, map[string]string{"status": "cleared"})
}

func (s *Server) handleGetStatus(c echo.Context) error {
	status := s.app.GetStatus()
	return c.JSON(http.StatusOK, status)
}

func (s *Server) handleGetWebUIConfig(c echo.Context) error {
	cfg := s.app.GetWebUI()
	// 不返回密码明文
	return c.JSON(http.StatusOK, config.WebUIConfig{
		Enabled:  cfg.Enabled,
		Port:     cfg.Port,
		Password: "",
	})
}

func (s *Server) handleUpdateWebUIConfig(c echo.Context) error {
	var body struct {
		Enabled  bool   `json:"enabled"`
		Port     int    `json:"port"`
		Password string `json:"password"`
	}
	if err := c.Bind(&body); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}

	cfg := config.WebUIConfig{
		Enabled:  body.Enabled,
		Port:     body.Port,
		Password: body.Password,
	}

	// 如果密码为空，保留原密码
	if cfg.Password == "" {
		current := s.app.GetWebUI()
		cfg.Password = current.Password
	}

	if err := s.app.SetWebUI(cfg); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": err.Error()})
	}
	return c.JSON(http.StatusOK, map[string]string{"status": "updated"})
}
