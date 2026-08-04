#!/bin/bash

echo "Overlay Fixed! Testing Centered Bottom Positioning..."
echo "================================================="

echo ""
echo "The app is now running. Press your recording hotkey to test the HUD."
echo ""
echo "Expected behavior:"
echo "  - HUD appears CENTERED horizontally on screen"
echo "  - HUD positioned 100px from BOTTOM edge"
echo "  - Works on fullscreen app spaces"
echo ""
echo "The helper computes its own position. To check it:"
echo ""
echo "1. Open Console.app and filter for 'GhostWriterOverlay'"
echo "2. Look for the 'Centering HUD on visible frame ... -> x,y' line"
echo "3. AppKit uses a BOTTOM-LEFT origin, so the expected values are:"
echo "     x = visible.origin.x + (visible.width - 220) / 2"
echo "     y = visible.origin.y + 100"
echo ""
echo "origin is NOT (0,0) once a second display is attached — it is often"
echo "negative. A y computed as 'height - 160' puts the HUD near the TOP."
echo ""
echo "For example, on a display whose visible frame is -1871,900 2560x1050:"
echo "  x = -1871 + (2560 - 220) / 2 = -701"
echo "  y = 900 + 100                = 1000"
echo ""
echo "To stop watching, press Ctrl+C"
echo ""

# Watch logs in real-time
tail -f /tmp/tauri_output.log 2>/dev/null | grep --line-buffered -E "Positioning|Screen|Window|Final|Calculated|Setting" &
TAIL_PID=$!

trap "kill $TAIL_PID 2>/dev/null; echo ''; exit 0" SIGINT SIGTERM

wait $TAIL_PID
