#!/bin/bash
# PortHannis / PortCLI development environment.
# Usage: source setup-env.sh

if [ -d /mnt/d/Code/Go/go ]; then
  export GOROOT=/mnt/d/Code/Go/go
  export GOPATH=/mnt/d/Code/Go/go-packages
else
  export GOROOT=/d/Code/Go/go
  export GOPATH=/d/Code/Go/go-packages
fi
export PATH="$GOROOT/bin:$GOPATH/bin:$PATH"

echo "PortCLI development environment loaded:"
echo "  GOROOT=$GOROOT"
echo "  GOPATH=$GOPATH"
if command -v go >/dev/null 2>&1; then
  echo "  Go version: $(go version)"
elif [ -x "$GOROOT/bin/go.exe" ]; then
  echo "  Go executable: $GOROOT/bin/go.exe"
else
  echo "  Go executable not found on PATH"
fi
