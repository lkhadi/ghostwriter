#!/bin/bash

echo "=== Testing Overlay Helper Directly ==="
echo ""

# Test helper in standalone mode
echo "1. Launching helper directly..."
./overlay-helper/GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper > /tmp/overlay_standalone.log 2>&1 &
HELPER_PID=$!

sleep 2

# Check if helper is running
if ps -p $HELPER_PID > /dev/null 2>&1; then
    echo "✓ Helper process started (PID: $HELPER_PID)"

    # Check for socket
    if [ -S "/tmp/ghostwriter_overlay.sock" ]; then
        echo "✓ Socket created"

        # Test SHOW command
        echo ""
        echo "2. Sending SHOW command to test positioning..."
        echo "SHOW 610 715" | nc -U /tmp/ghostwriter_overlay.sock

        if [ $? -eq 0 ]; then
            echo "✓ SHOW command sent successfully"
            echo ""
            echo "⏱️  HUD should now be visible."
            echo "   Check Console.app (filter: GhostWriterOverlay) for debug output"
            echo "   Expected position: Centered horizontally, 100px from bottom"
            echo ""
            echo "Press Enter to hide HUD..."
            read

            # Test HIDE command
            echo "Sending HIDE command..."
            echo "HIDE" | nc -U /tmp/ghostwriter_overlay.sock

            if [ $? -eq 0 ]; then
                echo "✓ HIDE command sent successfully"
            else
                echo "✗ HIDE command failed"
            fi
        else
            echo "✗ Failed to send SHOW command"
            echo ""
            echo "Socket log:"
            cat /tmp/overlay_standalone.log
        fi
    else
        echo "✗ Socket not created"
        echo ""
        echo "Helper log:"
        cat /tmp/overlay_standalone.log
        ps -p $HELPER_PID || echo "Process not running"
    fi
else
    echo "✗ Helper failed to start"
    echo ""
    echo "Helper log:"
    cat /tmp/overlay_standalone.log
fi

# Test QUIT
echo ""
echo "3. Testing QUIT command..."
echo "QUIT" | nc -U /tmp/ghostwriter_overlay.sock 2>/dev/null
sleep 1

if ! ps -p $HELPER_PID > /dev/null 2>&1; then
    echo "✓ Helper quit successfully"
else
    echo "Helper still running, manual kill needed"
    kill $HELPER_PID 2>/dev/null
fi

echo ""
echo "=== Test Complete ==="
echo "Check Console.app (filter: GhostWriterOverlay) for detailed debug output"
