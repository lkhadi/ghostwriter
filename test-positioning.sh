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
echo "⚠️  If HUD still appears near top, check these things:"
echo ""
echo "1. Open Console.app and filter for 'GhostWriterOverlay' to see debug output"
echo "2. Look for 'Screen visible frame' and 'Final frame' messages"
echo "3. The Y coordinate should be close to screen height - 160 (100px margin)"
echo ""
echo "Your screen resolution should show in the logs."
echo ""
echo "For example, on a 1440x900 screen with 25px menu bar:"
echo "  Visible frame height: 875"
echo "  Expected Y position: 875 - 60 - 100 = 715"
echo ""
echo "To stop watching, press Ctrl+C"
echo ""

# Watch logs in real-time
tail -f /tmp/tauri_output.log 2>/dev/null | grep --line-buffered -E "Positioning|Screen|Window|Final|Calculated|Setting" &
TAIL_PID=$!

trap "kill $TAIL_PID 2>/dev/null; echo ''; exit 0" SIGINT SIGTERM

wait $TAIL_PID
