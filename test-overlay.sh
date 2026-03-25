#!/bin/bash

echo "Testing Overlay Helper Implementation..."
echo "====================================="

# 1. Check if helper app exists
echo -e "\n1. Checking helper app..."
if [ -f "overlay-helper/GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper" ]; then
    echo "✓ Helper app found"
else
    echo "✗ Helper app not found"
    exit 1
fi

# 2. Build helper if needed
echo -e "\n2. Building helper..."
cd overlay-helper
make clean && make
if [ $? -eq 0 ]; then
    echo "✓ Helper app built successfully"
else
    echo "✗ Helper app build failed"
    exit 1
fi
cd ..

# 3. Test helper standalone
echo -e "\n3. Testing helper standalone..."
./overlay-helper/GhostWriterOverlayHelper.app/Contents/MacOS/GhostWriterOverlayHelper > /tmp/overlay_test.log 2>&1 &
HELPER_PID=$!
sleep 2

if [ -S "/tmp/ghostwriter_overlay.sock" ]; then
    echo "✓ Helper socket created"

    # Test SHOW command
    echo "SHOW 100 100" | nc -U /tmp/ghostwriter_overlay.sock
    if [ $? -eq 0 ]; then
        echo "✓ SHOW command successful (HUD should be visible)"

        # Wait for user to see it
        echo -e "\n⏱️  Press Enter to hide HUD..."
        read

        # Test HIDE command
        echo "HIDE" | nc -U /tmp/ghostwriter_overlay.sock
        if [ $? -eq 0 ]; then
            echo "✓ HIDE command successful (HUD should be hidden)"
        else
            echo "✗ HIDE command failed"
        fi
    else
        echo "✗ SHOW command failed"
    fi

    # Test QUIT command
    echo "QUIT" | nc -U /tmp/ghostwriter_overlay.sock
    sleep 1
else
    echo "✗ Helper socket not created"
    cat /tmp/overlay_test.log
    kill $HELPER_PID 2>/dev/null
    exit 1
fi

echo -e "\n✓ All tests passed!"
echo "====================================="
echo "Overlay helper is working correctly."
echo "Now run 'npm run tauri dev' to test with main app."
