package webui

import (
	"context"
	"embed"
	"fmt"
	"io/fs"
	"log"
	"net/http"

	"github.com/HannisLee/PortHannis/config"
	"github.com/labstack/echo/v4"
)

// Server WebUI HTTP 服务器
type Server struct {
	echo *echo.Echo
	app  *AppBindings
	addr string
}

// AppBindings 主应用提供的回调集合，避免循环依赖
type AppBindings struct {
	GetRules    func() []config.ForwardRule
	AddRule     func(sourceHost string, localPort int, targetHost string, targetPort int) error
	DeleteRule  func(id string) error
	ToggleRule  func(id string, enabled bool) error
	GetLogs     func(ruleID string, limit int) []config.LogEntry
	ClearLogs   func(ruleID string) error
	GetStatus   func() map[string]bool
	GetWebUI    func() config.WebUIConfig
	SetWebUI    func(cfg config.WebUIConfig) error
}

// FrontendAssets 前端静态文件
var frontendAssets embed.FS

// SetFrontendAssets 设置前端静态文件
func SetFrontendAssets(assets embed.FS) {
	frontendAssets = assets
}

// NewServer 创建 WebUI 服务器
func NewServer(bindings *AppBindings, port int) *Server {
	e := echo.New()
	e.HideBanner = true
	e.HidePort = true

	s := &Server{
		echo: e,
		app:  bindings,
		addr: fmt.Sprintf(":%d", port),
	}

	s.setupAuth()
	s.registerAPI()
	s.registerWebSocket()
	s.serveFrontend()

	return s
}

// Start 启动 HTTP 服务器（非阻塞）
func (s *Server) Start() {
	go func() {
		if err := s.echo.Start(s.addr); err != nil && err != http.ErrServerClosed {
			log.Printf("WebUI server error: %v", err)
		}
	}()
	log.Printf("WebUI server listening on http://localhost%s", s.addr)
}

// Stop 优雅关闭
func (s *Server) Stop(ctx context.Context) error {
	return s.echo.Shutdown(ctx)
}

// serveFrontend 提供前端静态文件
func (s *Server) serveFrontend() {
	sub, err := fs.Sub(frontendAssets, "frontend/dist")
	if err != nil {
		log.Printf("Failed to create sub FS for frontend: %v", err)
		return
	}
	assetHandler := http.FileServer(http.FS(sub))
	s.echo.GET("/*", echo.WrapHandler(http.StripPrefix("/", assetHandler)))
}
