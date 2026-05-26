package main

import (
	"bufio"
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"text/tabwriter"
	"time"

	"github.com/HannisLee/PortHannis/config"
	"github.com/HannisLee/PortHannis/forwarder"
)

const (
	internalDaemonCommand = "__daemon"
	controlTokenHeader    = "X-PortCLI-Token"
)

type daemonState struct {
	PID   int    `json:"pid"`
	Port  int    `json:"port"`
	Token string `json:"token"`
}

type ruleStatus struct {
	Name    string `json:"name"`
	Enabled bool   `json:"enabled"`
	Running bool   `json:"running"`
	Error   string `json:"error,omitempty"`
}

type daemonStatus struct {
	Running bool         `json:"running"`
	PID     int          `json:"pid"`
	Rules   []ruleStatus `json:"rules"`
}

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(1)
	}

	var err error
	switch os.Args[1] {
	case "add":
		err = cmdAdd(os.Args[2:])
	case "list":
		err = cmdList(os.Args[2:])
	case "enable":
		err = cmdSetEnabled(os.Args[2:], true)
	case "disable":
		err = cmdSetEnabled(os.Args[2:], false)
	case "remove":
		err = cmdRemove(os.Args[2:])
	case "run":
		err = cmdRun(os.Args[2:])
	case "status":
		err = cmdStatus(os.Args[2:])
	case "stop":
		err = cmdStop(os.Args[2:])
	case "logs":
		err = cmdLogs(os.Args[2:])
	case "clear-logs":
		err = cmdClearLogs(os.Args[2:])
	case "config":
		err = cmdConfig(os.Args[2:])
	case internalDaemonCommand:
		err = cmdDaemon(os.Args[2:])
	case "-h", "--help", "help":
		usage()
	default:
		err = fmt.Errorf("unknown command %q", os.Args[1])
	}

	if err != nil {
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}

func usage() {
	fmt.Println(`portcli - cross-platform TCP port forwarding

Usage:
  portcli add --listen 0.0.0.0:8080 --target 192.168.1.100:3000
  portcli add --name web --listen 127.0.0.1:9000 --target 10.0.0.5:22
  portcli list
  portcli enable <name>
  portcli disable <name>
  portcli remove <name>
  portcli run
  portcli status
  portcli stop
  portcli logs <name> [--limit 100] [--follow]
  portcli clear-logs <name>
  portcli config path`)
}

func cmdAdd(args []string) error {
	fs := flag.NewFlagSet("add", flag.ContinueOnError)
	name := fs.String("name", "", "rule name")
	listen := fs.String("listen", "", "listen address host:port")
	target := fs.String("target", "", "target address host:port")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if *listen == "" || *target == "" {
		return errors.New("add requires --listen and --target")
	}

	sourceHost, localPort, err := parseAddress(*listen)
	if err != nil {
		return fmt.Errorf("invalid --listen: %w", err)
	}
	if sourceHost == "" {
		sourceHost = "0.0.0.0"
	}
	targetHost, targetPort, err := parseAddress(*target)
	if err != nil {
		return fmt.Errorf("invalid --target: %w", err)
	}
	if targetHost == "" {
		return errors.New("target host cannot be empty")
	}

	mgr, err := loadManager()
	if err != nil {
		return err
	}
	addedName := *name
	if addedName == "" {
		addedName = nextAvailableName(mgr.Rules())
	}
	rule := config.Rule{
		SourceHost: sourceHost,
		LocalPort:  localPort,
		TargetHost: targetHost,
		TargetPort: targetPort,
		Enabled:    true,
	}
	if err := mgr.AddRule(addedName, rule); err != nil {
		return err
	}

	fmt.Printf("added rule %s\n", addedName)
	return nil
}

func cmdList(args []string) error {
	fs := flag.NewFlagSet("list", flag.ContinueOnError)
	if err := fs.Parse(args); err != nil {
		return err
	}
	mgr, err := loadManager()
	if err != nil {
		return err
	}
	rules := mgr.Rules()
	names := sortedRuleNames(rules)
	if len(names) == 0 {
		fmt.Println("no rules configured")
		return nil
	}

	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "NAME\tLISTEN\tTARGET\tENABLED\tLOG PATH")
	for _, name := range names {
		rule := rules[name]
		fmt.Fprintf(w, "%s\t%s\t%s\t%t\t%s\n",
			name,
			net.JoinHostPort(rule.SourceHost, strconv.Itoa(rule.LocalPort)),
			net.JoinHostPort(rule.TargetHost, strconv.Itoa(rule.TargetPort)),
			rule.Enabled,
			rule.LogPath,
		)
	}
	return w.Flush()
}

func cmdSetEnabled(args []string, enabled bool) error {
	if len(args) != 1 {
		if enabled {
			return errors.New("enable requires a rule name")
		}
		return errors.New("disable requires a rule name")
	}
	mgr, err := loadManager()
	if err != nil {
		return err
	}
	if err := mgr.SetEnabled(args[0], enabled); err != nil {
		return err
	}
	if enabled {
		fmt.Printf("enabled %s\n", args[0])
	} else {
		fmt.Printf("disabled %s\n", args[0])
	}
	return nil
}

func cmdRemove(args []string) error {
	if len(args) != 1 {
		return errors.New("remove requires a rule name")
	}
	mgr, err := loadManager()
	if err != nil {
		return err
	}
	if err := mgr.DeleteRule(args[0]); err != nil {
		return err
	}
	fmt.Printf("removed %s\n", args[0])
	return nil
}

func cmdRun(args []string) error {
	fs := flag.NewFlagSet("run", flag.ContinueOnError)
	if err := fs.Parse(args); err != nil {
		return err
	}

	state, status, err := queryDaemon()
	if err == nil && status.Running {
		fmt.Printf("portcli daemon already running (pid %d)\n", status.PID)
		return nil
	}
	if state != nil {
		_ = removeState()
	}

	token, err := randomToken()
	if err != nil {
		return err
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(config.GetConfigDir(), 0755); err != nil {
		return err
	}
	logPath := filepath.Join(config.GetConfigDir(), "daemon.log")
	logFile, err := os.OpenFile(logPath, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0644)
	if err != nil {
		return err
	}
	defer logFile.Close()

	cmd := exec.Command(exe, internalDaemonCommand, "--token", token)
	cmd.Stdout = logFile
	cmd.Stderr = logFile
	if err := cmd.Start(); err != nil {
		return err
	}

	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		state, status, err := queryDaemon()
		if err == nil && state != nil && state.Token == token && status.Running {
			fmt.Printf("portcli daemon started (pid %d)\n", status.PID)
			return nil
		}
		time.Sleep(100 * time.Millisecond)
	}

	_ = cmd.Process.Kill()
	return fmt.Errorf("daemon did not become ready; see %s", logPath)
}

func cmdStatus(args []string) error {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
	if err := fs.Parse(args); err != nil {
		return err
	}
	state, status, err := queryDaemon()
	if err != nil {
		if state != nil {
			_ = removeState()
			fmt.Println("portcli daemon is not running (removed stale state)")
			return nil
		}
		fmt.Println("portcli daemon is not running")
		return nil
	}

	fmt.Printf("portcli daemon is running (pid %d)\n", status.PID)
	if len(status.Rules) == 0 {
		fmt.Println("no enabled rules")
		return nil
	}

	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "NAME\tENABLED\tRUNNING\tERROR")
	for _, rule := range status.Rules {
		fmt.Fprintf(w, "%s\t%t\t%t\t%s\n", rule.Name, rule.Enabled, rule.Running, rule.Error)
	}
	return w.Flush()
}

func cmdStop(args []string) error {
	fs := flag.NewFlagSet("stop", flag.ContinueOnError)
	if err := fs.Parse(args); err != nil {
		return err
	}
	state, _, err := queryDaemon()
	if err != nil {
		if state != nil {
			_ = removeState()
			fmt.Println("portcli daemon is not running (removed stale state)")
			return nil
		}
		fmt.Println("portcli daemon is not running")
		return nil
	}

	req, err := http.NewRequest(http.MethodPost, controlURL(state, "/stop"), nil)
	if err != nil {
		return err
	}
	req.Header.Set(controlTokenHeader, state.Token)
	resp, err := (&http.Client{Timeout: 3 * time.Second}).Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("stop failed: %s", strings.TrimSpace(string(body)))
	}
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if _, _, err := queryDaemon(); err != nil {
			fmt.Println("portcli daemon stopped")
			return nil
		}
		time.Sleep(100 * time.Millisecond)
	}
	fmt.Println("portcli daemon stopped")
	return nil
}

func cmdLogs(args []string) error {
	fs := flag.NewFlagSet("logs", flag.ContinueOnError)
	limit := fs.Int("limit", 100, "number of latest log entries")
	follow := fs.Bool("follow", false, "follow appended log entries")
	name := ""
	parseArgs := args
	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		name = args[0]
		parseArgs = args[1:]
	}
	if err := fs.Parse(parseArgs); err != nil {
		return err
	}
	if name == "" {
		if fs.NArg() != 1 {
			return errors.New("logs requires a rule name")
		}
		name = fs.Arg(0)
	} else if fs.NArg() != 0 {
		return errors.New("logs accepts one rule name")
	}
	mgr, err := loadManager()
	if err != nil {
		return err
	}
	rule, ok := mgr.GetRule(name)
	if !ok {
		return fmt.Errorf("rule %q not found", name)
	}
	if rule.LogPath == "" {
		return fmt.Errorf("rule %q has no logPath", name)
	}

	entries, err := forwarder.ReadLogFile(rule.LogPath, *limit)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		printLogEntry(entry)
	}
	if *follow {
		return followLog(rule.LogPath)
	}
	return nil
}

func cmdClearLogs(args []string) error {
	if len(args) != 1 {
		return errors.New("clear-logs requires a rule name")
	}
	mgr, err := loadManager()
	if err != nil {
		return err
	}
	rule, ok := mgr.GetRule(args[0])
	if !ok {
		return fmt.Errorf("rule %q not found", args[0])
	}
	if rule.LogPath == "" {
		return fmt.Errorf("rule %q has no logPath", args[0])
	}
	if err := forwarder.ClearLogFile(rule.LogPath); err != nil {
		return err
	}
	fmt.Printf("cleared logs for %s\n", args[0])
	return nil
}

func cmdConfig(args []string) error {
	if len(args) != 1 || args[0] != "path" {
		return errors.New("config supports only: portcli config path")
	}
	mgr := config.NewManager()
	fmt.Println(mgr.ConfigPath())
	return nil
}

func cmdDaemon(args []string) error {
	fs := flag.NewFlagSet(internalDaemonCommand, flag.ContinueOnError)
	token := fs.String("token", "", "control token")
	if err := fs.Parse(args); err != nil {
		return err
	}
	if *token == "" {
		return errors.New("missing daemon token")
	}
	return runDaemon(*token)
}

func runDaemon(token string) error {
	mgr, err := loadManager()
	if err != nil {
		return err
	}
	if _, err := mgr.EnsureLogPaths(); err != nil {
		return err
	}

	engine := forwarder.NewEngine()
	startErrors := map[string]string{}
	startEnabledRules(mgr, engine, startErrors)

	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		engine.StopAll()
		return err
	}
	defer listener.Close()

	_, portText, err := net.SplitHostPort(listener.Addr().String())
	if err != nil {
		engine.StopAll()
		return err
	}
	port, err := strconv.Atoi(portText)
	if err != nil {
		engine.StopAll()
		return err
	}

	state := daemonState{PID: os.Getpid(), Port: port, Token: token}
	if err := writeState(state); err != nil {
		engine.StopAll()
		return err
	}
	defer removeState()
	defer engine.StopAll()

	shutdown := make(chan struct{})
	server := &http.Server{Handler: controlHandler(token, mgr, engine, startErrors, shutdown)}
	serverDone := make(chan error, 1)
	go func() {
		err := server.Serve(listener)
		if errors.Is(err, http.ErrServerClosed) {
			err = nil
		}
		serverDone <- err
	}()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt)
	defer signal.Stop(sigCh)

	select {
	case <-shutdown:
	case <-sigCh:
	case err := <-serverDone:
		return err
	}

	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	_ = server.Shutdown(ctx)
	return <-serverDone
}

func controlHandler(token string, mgr *config.Manager, engine *forwarder.Engine, startErrors map[string]string, shutdown chan<- struct{}) http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/status", func(w http.ResponseWriter, r *http.Request) {
		if !validControlToken(r, token) {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		writeJSON(w, currentStatus(mgr, engine, startErrors))
	})
	mux.HandleFunc("/stop", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		if !validControlToken(r, token) {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		writeJSON(w, map[string]string{"status": "stopping"})
		go func() {
			time.Sleep(100 * time.Millisecond)
			shutdown <- struct{}{}
		}()
	})
	return mux
}

func startEnabledRules(mgr *config.Manager, engine *forwarder.Engine, startErrors map[string]string) {
	rules := mgr.Rules()
	for _, name := range sortedRuleNames(rules) {
		rule := rules[name]
		if !rule.Enabled {
			continue
		}
		if err := engine.StartRule(name, rule); err != nil {
			startErrors[name] = err.Error()
		}
	}
}

func currentStatus(mgr *config.Manager, engine *forwarder.Engine, startErrors map[string]string) daemonStatus {
	rules := mgr.Rules()
	running := engine.GetStatus()
	names := sortedRuleNames(rules)
	status := daemonStatus{Running: true, PID: os.Getpid()}
	for _, name := range names {
		rule := rules[name]
		if !rule.Enabled {
			continue
		}
		status.Rules = append(status.Rules, ruleStatus{
			Name:    name,
			Enabled: rule.Enabled,
			Running: running[name],
			Error:   startErrors[name],
		})
	}
	return status
}

func validControlToken(r *http.Request, token string) bool {
	return r.Header.Get(controlTokenHeader) == token
}

func queryDaemon() (*daemonState, daemonStatus, error) {
	state, err := readState()
	if err != nil {
		return nil, daemonStatus{}, err
	}
	req, err := http.NewRequest(http.MethodGet, controlURL(state, "/status"), nil)
	if err != nil {
		return state, daemonStatus{}, err
	}
	req.Header.Set(controlTokenHeader, state.Token)
	resp, err := (&http.Client{Timeout: 500 * time.Millisecond}).Do(req)
	if err != nil {
		return state, daemonStatus{}, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return state, daemonStatus{}, fmt.Errorf("daemon status returned %s", resp.Status)
	}
	var status daemonStatus
	if err := json.NewDecoder(resp.Body).Decode(&status); err != nil {
		return state, daemonStatus{}, err
	}
	return state, status, nil
}

func controlURL(state *daemonState, path string) string {
	return fmt.Sprintf("http://127.0.0.1:%d%s", state.Port, path)
}

func readState() (*daemonState, error) {
	data, err := os.ReadFile(config.GetStatePath())
	if err != nil {
		return nil, err
	}
	var state daemonState
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, err
	}
	if state.Port == 0 || state.Token == "" {
		return nil, errors.New("invalid daemon state")
	}
	return &state, nil
}

func writeState(state daemonState) error {
	if err := os.MkdirAll(config.GetConfigDir(), 0755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return os.WriteFile(config.GetStatePath(), data, 0600)
}

func removeState() error {
	err := os.Remove(config.GetStatePath())
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	return err
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(v)
}

func loadManager() (*config.Manager, error) {
	mgr := config.NewManager()
	if err := mgr.LoadConfig(); err != nil {
		return nil, err
	}
	return mgr, nil
}

func parseAddress(value string) (string, int, error) {
	host, portText, err := net.SplitHostPort(value)
	if err != nil {
		return "", 0, err
	}
	port, err := strconv.Atoi(portText)
	if err != nil {
		return "", 0, fmt.Errorf("invalid port %q", portText)
	}
	if port < 1 || port > 65535 {
		return "", 0, fmt.Errorf("port must be between 1 and 65535")
	}
	return host, port, nil
}

func sortedRuleNames(rules config.Config) []string {
	names := make([]string, 0, len(rules))
	for name := range rules {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func nextAvailableName(rules config.Config) string {
	for i := 1; ; i++ {
		name := "name" + strconv.Itoa(i)
		if _, exists := rules[name]; !exists {
			return name
		}
	}
}

func randomToken() (string, error) {
	var b [32]byte
	if _, err := rand.Read(b[:]); err != nil {
		return "", err
	}
	return hex.EncodeToString(b[:]), nil
}

func printLogEntry(entry config.LogEntry) {
	fmt.Printf("%s\t%s\t%d\t%d\t%s\n",
		entry.Timestamp.Format(time.RFC3339),
		entry.Source,
		entry.BytesIn,
		entry.BytesOut,
		entry.Status,
	)
}

func followLog(path string) error {
	f, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			for {
				time.Sleep(time.Second)
				f, err = os.Open(path)
				if err == nil {
					break
				}
				if !os.IsNotExist(err) {
					return err
				}
			}
		} else {
			return err
		}
	}
	defer f.Close()

	if _, err := f.Seek(0, io.SeekEnd); err != nil {
		return err
	}
	reader := bufio.NewReader(f)
	for {
		line, err := reader.ReadBytes('\n')
		if len(line) > 0 {
			var entry config.LogEntry
			if json.Unmarshal(bytes.TrimSpace(line), &entry) == nil {
				printLogEntry(entry)
			}
		}
		if err == nil {
			continue
		}
		if errors.Is(err, io.EOF) {
			time.Sleep(time.Second)
			continue
		}
		return err
	}
}
