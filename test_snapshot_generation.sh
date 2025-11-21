#!/bin/bash

echo "🧪 Testing Report Snapshot Generation"
echo "====================================="
echo ""

# Check if backend is already running
if lsof -Pi :3000 -sTCP:LISTEN -t >/dev/null 2>&1 ; then
    echo "⚠️  Port 3000 is already in use"
    echo "Please stop the existing report server first"
    exit 1
fi

# Start the report server in background
echo "1. Starting report server..."
cd /Users/xingao/mmexecutor
./target/release/mexc-grid-trader report --config config.json > /tmp/report_server.log 2>&1 &
SERVER_PID=$!
echo "   Server PID: $SERVER_PID"

# Wait for server to start
echo "2. Waiting for server to initialize..."
sleep 3

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "   ❌ Server failed to start"
    cat /tmp/report_server.log
    exit 1
fi

echo "   ✅ Server is running"

# Check initial state
echo ""
echo "3. Checking initial snapshot files..."
SNAPSHOT_FILES=$(find . -maxdepth 1 -name ".snapshots_*" 2>/dev/null | wc -l)
echo "   Found $SNAPSHOT_FILES snapshot file(s)"

# Wait for first update cycle (30 seconds)
echo ""
echo "4. Waiting for first snapshot cycle (35 seconds)..."
for i in {35..1}; do
    printf "\r   ⏳ %2d seconds remaining..." $i
    sleep 1
done
echo ""

# Check snapshot files again
echo ""
echo "5. Checking for new snapshots..."
NEW_SNAPSHOT_FILES=$(find . -maxdepth 1 -name ".snapshots_*" 2>/dev/null)
NEW_COUNT=$(echo "$NEW_SNAPSHOT_FILES" | grep -v "^$" | wc -l)

if [ "$NEW_COUNT" -gt 0 ]; then
    echo "   ✅ Found $NEW_COUNT snapshot file(s):"
    for file in $NEW_SNAPSHOT_FILES; do
        SIZE=$(du -h "$file" | cut -f1)
        LINES=$(wc -l < "$file" 2>/dev/null || echo "0")
        echo "      - $file ($SIZE, $LINES snapshots)"

        # Show last snapshot
        if [ "$LINES" -gt 0 ]; then
            echo "      Latest snapshot:"
            tail -1 "$file" | python3 -m json.tool 2>/dev/null | head -10 | sed 's/^/        /'
        fi
    done
else
    echo "   ⚠️  No snapshot files created yet"
    echo "   This might be normal if:"
    echo "   - Exchanges are not configured properly"
    echo "   - API keys are invalid"
    echo "   - Network issues"
fi

# Check server logs
echo ""
echo "6. Server logs (last 10 lines):"
tail -10 /tmp/report_server.log | sed 's/^/   /'

# Test API endpoint
echo ""
echo "7. Testing API endpoint..."
REPORT_JSON=$(curl -s http://localhost:3000/api/report)
if [ $? -eq 0 ]; then
    echo "   ✅ API endpoint is working"

    # Check if PnL summary exists
    HAS_PNL=$(echo "$REPORT_JSON" | grep -o "pnl_summary" | wc -l)
    if [ "$HAS_PNL" -gt 0 ]; then
        echo "   ✅ PnL summary data exists"

        # Show PnL status for each exchange
        echo "$REPORT_JSON" | python3 -c "
import json
import sys
try:
    data = json.load(sys.stdin)
    for ex in data.get('exchanges', []):
        name = ex['name']
        pnl = ex.get('pnl_summary')
        if pnl:
            print(f'      ✅ {name}: Has PnL data')
        else:
            print(f'      ⚠️  {name}: No PnL data (need more snapshots)')
except:
    pass
" 2>/dev/null
    else
        echo "   ⚠️  No PnL summary yet (need at least 2 snapshots)"
    fi
else
    echo "   ❌ Failed to reach API endpoint"
fi

# Cleanup
echo ""
echo "8. Stopping test server..."
kill $SERVER_PID 2>/dev/null
sleep 1
if kill -0 $SERVER_PID 2>/dev/null; then
    kill -9 $SERVER_PID 2>/dev/null
fi
echo "   ✅ Server stopped"

echo ""
echo "📋 Summary"
echo "=========="
if [ "$NEW_COUNT" -gt 0 ]; then
    echo "✅ Report command is successfully generating snapshots!"
    echo "   Snapshot files are being created every 30 seconds"
    echo "   After 2+ snapshots, you'll see PnL summary and average price"
else
    echo "⚠️  No snapshots were generated"
    echo "   Check the logs above for errors"
    echo "   Verify your config.json has valid API credentials"
fi

echo ""
echo "🚀 To use in production:"
echo "   ./target/release/mexc-grid-trader report --config config.json"
echo ""
echo "   The server will continuously:"
echo "   - Update report data every 30 seconds"
echo "   - Save account snapshots"
echo "   - Calculate PnL and average trading prices"
