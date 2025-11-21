#!/bin/bash

echo "🔍 Frontend Diagnostics"
echo "======================"
echo ""

# Check if backend is running
echo "1. Checking backend API..."
if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo "   ✅ Backend is running"
else
    echo "   ❌ Backend is NOT running"
    echo "   Please start with: ./target/release/mexc-grid-trader report"
    exit 1
fi

echo ""
echo "2. Fetching report data..."
REPORT=$(curl -s http://localhost:3000/api/report)

# Check if report has data
if [ -z "$REPORT" ]; then
    echo "   ❌ No data returned from API"
    exit 1
fi

echo "   ✅ Report data received"

# Check for PnL summary
HAS_PNL=$(echo "$REPORT" | grep -o "pnl_summary" | wc -l)
echo ""
echo "3. Checking PnL summary..."
if [ "$HAS_PNL" -gt 0 ]; then
    echo "   ✅ PnL summary exists in data"

    # Extract base and quote changes
    echo ""
    echo "4. Analyzing asset changes..."
    echo "$REPORT" | python3 -c "
import json
import sys
data = json.load(sys.stdin)
for exchange in data.get('exchanges', []):
    print(f\"   Exchange: {exchange['name']}\")
    pnl = exchange.get('pnl_summary')
    if pnl:
        quote = pnl['quote_asset_summary']
        base = pnl['base_asset_summary']
        print(f\"   Quote ({quote['asset']}): {quote['absolute_change']:.2f}\")
        print(f\"   Base ({base['asset']}): {base['absolute_change']:.8f}\")

        if base['absolute_change'] != 0:
            avg_price = abs(quote['absolute_change'] / base['absolute_change'])
            print(f\"   ✅ Average Price: {avg_price:.6f}\")
            print(f\"   This SHOULD display in the UI!\")
        else:
            print(f\"   ⚠️  Base change is 0 - Average Price card will NOT show\")
            print(f\"   You need to wait for trades to happen!\")
    else:
        print(f\"   ❌ No PnL summary for this exchange\")
" 2>/dev/null || echo "   ⚠️  Install Python3 for detailed analysis"

else
    echo "   ❌ No PnL summary in data"
    echo "   You need at least 2 snapshots with different balances"
    echo "   Run 'watch' mode for a while to collect data"
fi

echo ""
echo "5. Frontend build status..."
if [ -f "frontend/build/index.html" ]; then
    echo "   ✅ Frontend is built"
    if grep -q "Average Trading Price" frontend/build/static/js/main.*.js 2>/dev/null; then
        echo "   ✅ New code is in the build"
    else
        echo "   ❌ New code NOT in build - run 'npm run build' in frontend/"
    fi
else
    echo "   ❌ Frontend not built - run 'npm run build' in frontend/"
fi

echo ""
echo "📋 Next Steps:"
echo "==============="
echo ""
echo "Option A: Use Development Server (recommended)"
echo "  1. cd frontend"
echo "  2. npm start"
echo "  3. Open http://localhost:3001 (or 3000)"
echo "  4. The page will auto-reload when you make changes"
echo ""
echo "Option B: Serve Built Files"
echo "  1. npm install -g serve"
echo "  2. serve -s frontend/build -p 8080"
echo "  3. Open http://localhost:8080"
echo ""
echo "💡 If you see the page but not the Average Price card:"
echo "  - Check that PnL summary exists (see above)"
echo "  - Make sure baseChange is not 0 (you need actual trades)"
echo "  - Try hard refresh: Ctrl+Shift+R (or Cmd+Shift+R on Mac)"
