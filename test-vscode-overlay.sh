#!/bin/bash

echo "=== Testing Overlay on VSCode Fullscreen ==="
echo ""

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# 1. Check if helper is running
if ! ps aux | grep -qi "GhostWriterOverlay" | grep -v grep > /dev/null; then
    echo "${RED}❌ Overlay helper not running${NC}"
    echo "Starting overlay helper..."
    cd overlay-helper
    make clean && make
    ./GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper > /tmp/overlay_vscode_test.log 2>&1 &
    HELPER_PID=$!
    echo "Helper PID: $HELPER_PID"
    sleep 2
fi

# 2. Check if socket exists
if [ ! -S "/tmp/ghostwriter_overlay.sock" ]; then
    echo "${RED}❌ Socket not found at /tmp/ghostwriter_overlay.sock${NC}"
    echo "Check logs: tail -f /tmp/overlay_vscode_test.log"
    exit 1
fi

echo "${GREEN}✓ Socket found${NC}"

# 3. Check if VSCode is running
if ! ps aux | grep -qi "Code\|Antigravity\|Electron" | grep -v grep > /dev/null; then
    echo "${YELLOW}⚠️  VSCode/Electron app not running${NC}"
    echo "Please start VSCode and put it in fullscreen, then press Enter to continue..."
    read
else
    echo "${GREEN}✓ VSCode/Electron app detected${NC}"
fi

echo ""
echo "=========================================="
echo "Phase 1: Testing NSMainMenuWindowLevel (Recommended for Electron/VSCode)"
echo "=========================================="
echo ""
echo "Setting window level to NSMainMenuWindowLevel..."
echo "SET_LEVEL MAIN" | nc -U /tmp/ghostwriter_overlay.sock
sleep 0.5

echo ""
echo "Showing overlay centered at bottom..."
echo "SHOW 610 715" | nc -U /tmp/ghostwriter_overlay.sock

echo ""
echo "If you see the HUD on VSCode fullscreen, press y"
echo "If NOT, press n"
read -p "HUD visible? (y/n): " answer

if [ "$answer" = "y" ]; then
    echo "${GREEN}✅ SUCCESS: NSMainMenuWindowLevel works!${NC}"
    echo ""
    echo "Press Enter to hide overlay..."
    read
    echo "HIDE" | nc -U /tmp/ghostwriter_overlay.sock

    echo ""
    echo "=== Test complete! ==="
    echo "Check Console.app (filter: GhostWriterOverlay) for detailed logs"
    exit 0
fi

echo ""
echo "=========================================="
echo "Phase 2: Testing NSFloatingWindowLevel (Alternative)"
echo "=========================================="
echo ""
echo "Setting window level to NSFloatingWindowLevel..."
echo "SET_LEVEL FLOATING" | nc -U /tmp/ghostwriter_overlay.sock
sleep 0.5

echo ""
echo "Showing overlay centered at bottom..."
echo "SHOW 610 715" | nc -U /tmp/ghostwriter_overlay.sock

echo ""
echo "If you see the HUD on VSCode fullscreen, press y"
echo "If NOT, press n"
read -p "HUD visible? (y/n): " answer

if [ "$answer" = "y" ]; then
    echo "${GREEN}✅ SUCCESS: NSFloatingWindowLevel works!${NC}"
    echo ""
    echo "Press Enter to hide overlay..."
    read
    echo "HIDE" | nc -U /tmp/ghostwriter_overlay.sock

    echo ""
    echo "=== Test complete! ==="
    echo "Check Console.app (filter: GhostWriterOverlay) for detailed logs"
    exit 0
else
    echo "${RED}❌ FAILED: Neither level works${NC}"
    echo ""
    echo "Troubleshooting steps:"
    echo "1. Check Console.app for errors (filter: GhostWriterOverlay)"
    echo "2. Verify overlay helper is running: ps aux | grep GhostWriterOverlay"
    echo "3. Check socket exists: ls -la /tmp/ghostwriter_overlay.sock"
    echo "4. Ensure VSCode is actually in fullscreen mode (Cmd+Ctrl+F)"
    echo "5. Try running: cat /tmp/overlay_vscode_test.log"
    echo ""
    echo "If issues persist, report:"
    echo "- Window level used: Check logs for 'Window Level:' line"
    echo "- Collection behavior: Check logs for 'Collection Behavior:' line"
    echo "- Space monitoring: Look for 'Active space changed' messages"
    exit 1
fi
