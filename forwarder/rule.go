package forwarder

import (
	"fmt"
	"io"
	"net"
	"sync"
	"time"

	"github.com/HannisLee/PortHannis/config"
)

type RuleState struct {
	name     string
	rule     config.Rule
	logger   *CircularLogger
	listener net.Listener
	quit     chan struct{}
	wg       sync.WaitGroup
	running  bool
	mu       sync.Mutex
}

func NewRuleState(name string, rule config.Rule, logger *CircularLogger) *RuleState {
	return &RuleState{
		name:   name,
		rule:   rule,
		logger: logger,
		quit:   make(chan struct{}),
	}
}

func (r *RuleState) Running() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.running
}

func (r *RuleState) Start() error {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.running {
		return fmt.Errorf("rule %q is already running", r.name)
	}

	addr := net.JoinHostPort(r.rule.SourceHost, fmt.Sprintf("%d", r.rule.LocalPort))
	if r.rule.SourceHost == "" || r.rule.SourceHost == "*" {
		addr = fmt.Sprintf(":%d", r.rule.LocalPort)
	}
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		return fmt.Errorf("failed to listen on %s: %w", addr, err)
	}
	r.listener = ln
	r.running = true
	r.quit = make(chan struct{})

	r.wg.Add(1)
	go r.acceptLoop()
	return nil
}

func (r *RuleState) Stop() {
	r.mu.Lock()
	if !r.running {
		r.mu.Unlock()
		return
	}
	r.running = false
	close(r.quit)
	if r.listener != nil {
		_ = r.listener.Close()
	}
	r.mu.Unlock()

	r.wg.Wait()
}

func (r *RuleState) acceptLoop() {
	defer r.wg.Done()

	for {
		conn, err := r.listener.Accept()
		if err != nil {
			select {
			case <-r.quit:
				return
			default:
				continue
			}
		}
		r.wg.Add(1)
		go func() {
			defer r.wg.Done()
			r.handleConnection(conn)
		}()
	}
}

func (r *RuleState) handleConnection(clientConn net.Conn) {
	defer clientConn.Close()

	targetAddr := net.JoinHostPort(r.rule.TargetHost, fmt.Sprintf("%d", r.rule.TargetPort))
	targetConn, err := net.DialTimeout("tcp", targetAddr, 10*time.Second)
	if err != nil {
		_ = r.logger.Write(config.LogEntry{
			Timestamp: time.Now(),
			Source:    clientConn.RemoteAddr().String(),
			Status:    "error",
		})
		return
	}
	defer targetConn.Close()

	entry := config.LogEntry{
		Timestamp: time.Now(),
		Source:    clientConn.RemoteAddr().String(),
		Status:    "connected",
	}

	var bytesIn, bytesOut int64
	done := make(chan struct{})

	go func() {
		bytesIn, _ = io.Copy(targetConn, clientConn)
		_ = targetConn.Close()
		close(done)
	}()

	bytesOut, _ = io.Copy(clientConn, targetConn)
	_ = clientConn.Close()
	<-done

	entry.BytesIn = bytesIn
	entry.BytesOut = bytesOut
	entry.Status = "closed"
	_ = r.logger.Write(entry)
}
