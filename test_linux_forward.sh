#!/bin/bash
B=/home/lihan/portcli/target/release/portcli

echo "=== L51: TCP Forwarding E2E Test ==="

# Make sure the rule is configured correctly
echo "Configuring test rule..."
$B modify web --target 127.0.0.1:8080 2>/dev/null
$B enable web 2>/dev/null

# Start daemon (stop old one first if any)
$B stop 2>/dev/null
sleep 0.5
rm -f ~/.local/share/portcli/state.json 2>/dev/null
$B run
sleep 1

echo "Daemon status:"
$B status | head -10
echo ""

# Start backend listener in background (listens on 8080)
echo "Starting backend listener on :8080..."
nc -l -p 8080 -w 10 > /tmp/backend_recv.txt &
BACKEND_PID=$!
sleep 0.5

# Send data through forwarded port 8081 -> 8080
echo "Connecting via portcli (8081 -> 8080)..."
echo "PING_FROM_LINUX_CLIENT" | nc -w 2 127.0.0.1 8081 > /tmp/client_recv.txt 2>&1 &
CLIENT_PID=$!
sleep 1

# Wait for client
wait $CLIENT_PID 2>/dev/null
kill $BACKEND_PID 2>/dev/null
wait $BACKEND_PID 2>/dev/null

echo ""
echo "Client received:"
cat /tmp/client_recv.txt 2>/dev/null || echo "(nothing)"
echo ""

echo "Forwarding logs:"
$B logs web -n 5
echo ""

echo "=== Forwarding E2E test complete ==="
