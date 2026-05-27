#!/bin/bash
set -e
B=/home/lihan/portcli/target/release/portcli

# Cleanup
rm -rf ~/.config/portcli ~/.local/share/portcli 2>/dev/null

echo "============================================"
echo "  portcli Linux Test Suite"
echo "  Platform: $(uname -a)"
echo "============================================"
echo ""

echo "=== L1: version ==="
$B --version
echo ""

echo "=== L2: help ==="
$B --help
echo ""

echo "=== L3: list (empty config) ==="
$B list
echo ""

echo "=== L4: add rules ==="
$B add web --source 0.0.0.0:8080 --target 192.168.31.10:8080
$B add ssh --source 127.0.0.1:2222 --target 192.168.31.20:22
echo ""

echo "=== L5: add duplicate ==="
$B add web --source 0.0.0.0:9090 --target 10.0.0.1:9090 2>&1 || true
echo ""

echo "=== L6: list (2 rules) ==="
$B list
echo ""

echo "=== L7: modify source ==="
$B modify web --source 0.0.0.0:8081
echo ""

echo "=== L8: modify target ==="
$B modify web --target 192.168.31.20:8080
echo ""

echo "=== L9: modify no changes ==="
$B modify web 2>&1 || true
echo ""

echo "=== L10: modify non-existent ==="
$B modify ghost --source 0.0.0.0:9999 2>&1 || true
echo ""

echo "=== L11: list (verify modify) ==="
$B list
echo ""

echo "=== L12: remove non-existent ==="
$B remove ghost 2>&1 || true
echo ""

echo "=== L13: remove ssh ==="
$B remove ssh
echo ""

echo "=== L14: list (1 rule left) ==="
$B list
echo ""

echo "=== L15: invalid address (bad port) ==="
$B add bad --source 0.0.0.0:99999 --target 127.0.0.1:80 2>&1 || true
echo ""

echo "=== L16: invalid address (no port) ==="
$B add bad2 --source 0.0.0.0 --target 127.0.0.1:80 2>&1 || true
echo ""

echo "=== L17: enable web ==="
$B enable web
echo ""

echo "=== L18: enable non-existent ==="
$B enable ghost 2>&1 || true
echo ""

echo "=== L19: disable web ==="
$B disable web
echo ""

echo "=== L20: disable non-existent ==="
$B disable ghost 2>&1 || true
echo ""

echo "=== L21: re-enable web ==="
$B enable web
echo ""

echo "=== L22: list (enabled) ==="
$B list
echo ""

echo "=== L23: add second rule for multi-rule test ==="
$B add db --source 127.0.0.1:5432 --target 192.168.31.50:5432
echo ""

echo "=== L24: status (daemon not running) ==="
$B status
echo ""

echo "=== L25: start background daemon ==="
$B run
sleep 1
echo ""

echo "=== L26: double start prevention ==="
$B run
echo ""

echo "=== L27: status (daemon running) ==="
$B status
echo ""

echo "=== L28: enable db + auto-reload ==="
$B enable db
sleep 1
echo ""

echo "=== L29: status (both rules) ==="
$B status
echo ""

echo "=== L30: disable db + auto-reload ==="
$B disable db
sleep 1
echo ""

echo "=== L31: status (db removed) ==="
$B status
echo ""

echo "=== L32: modify web target to localhost ==="
$B modify web --target 127.0.0.1:8080
sleep 1
echo ""

echo "=== L33: status (verify reload) ==="
$B status
echo ""

echo "=== L34: daemon logs ==="
$B logs -n 10
echo ""

echo "=== L35: web rule logs ==="
$B logs web -n 10
echo ""

echo "=== L36: logs -n 3 ==="
$B logs web -n 3
echo ""

echo "=== L37: logs --dir ==="
$B logs --dir
echo ""

echo "=== L38: logs non-existent rule ==="
$B logs ghost 2>&1 || true
echo ""

echo "=== L39: logs --clear web ==="
$B logs web --clear
echo ""

echo "=== L40: verify web log cleared ==="
$B logs web
echo ""

echo "=== L41: logs --clear daemon ==="
$B logs --clear
echo ""

echo "=== L42: verify daemon log cleared ==="
$B logs
echo ""

echo "=== L43: stop daemon ==="
$B stop
sleep 1
echo ""

echo "=== L44: stop when not running ==="
$B stop
echo ""

echo "=== L45: reload when not running ==="
$B reload
echo ""

echo "=== L46: status after stop ==="
$B status
echo ""

echo "=== L47: verify state file deleted ==="
if [ -f ~/.local/share/portcli/state.json ]; then
    echo "STATE FILE EXISTS (BUG)"
else
    echo "STATE FILE DELETED (CORRECT)"
fi
echo ""

echo "=== L48: config file location ==="
echo "Config:"
ls -la ~/.config/portcli/config/config.toml 2>/dev/null || echo "(no config)"
echo ""
echo "Log dir:"
find ~/.local/share/portcli/logs -type f 2>/dev/null || echo "(no logs)"
echo ""

echo "=== L49: foreground daemon ==="
echo "(Starting foreground daemon for 3 seconds...)"
timeout 3 $B run --foreground 2>/dev/null || true
echo ""

echo "=== L50: verify stopped after foreground timeout ==="
$B status
echo ""

echo "============================================"
echo "  All Linux tests completed"
echo "============================================"
