# Report Dashboard - REST API & React Frontend

The `report` command has been redesigned as a REST API service with a React frontend dashboard to provide real-time trading information.

## Architecture

### Backend (Rust)
- **Framework**: Axum (async web framework)
- **API Endpoints**:
  - `GET /api/report` - Returns comprehensive trading report data in JSON format
  - `GET /health` - Health check endpoint
- **Features**:
  - Background data refresh every 30 seconds
  - CORS enabled for frontend access
  - Caches report data for fast responses

### Frontend (React + TypeScript)
- **Framework**: React 18 with TypeScript
- **Styling**: Custom CSS with responsive design
- **Features**:
  - Auto-refresh every 30 seconds
  - Real-time display of all trading metrics
  - Mobile-responsive layout
  - Beautiful gradient UI

## Usage

### 1. Start the Backend API Server

```bash
# Build the project
cargo build --release

# Start the API server (default port 3000)
./target/release/mexc-grid-trader report

# Or specify a custom port
./target/release/mexc-grid-trader report --port 8080

# Or specify a config file
./target/release/mexc-grid-trader report --config config.json --port 3000
```

The server will start and display:
```
🚀 API Server starting on http://0.0.0.0:3000
📊 Report endpoint: http://localhost:3000/api/report
💚 Health check: http://localhost:3000/health
⚠️  Press Ctrl+C to stop
```

### 2. Start the React Frontend

```bash
# Navigate to frontend directory
cd frontend

# Install dependencies (first time only)
npm install

# Start the development server
npm start
```

The React app will open in your browser at `http://localhost:3000` (or the next available port).

### 3. Build Frontend for Production

```bash
cd frontend

# Create production build
npm run build

# The build output will be in frontend/build/
# You can serve it with any static file server
```

## API Response Format

The `/api/report` endpoint returns JSON data in the following format:

```json
{
  "timestamp": "2025-11-12T13:45:00Z",
  "exchanges": [
    {
      "name": "Gate.io",
      "account_info": {
        "base_asset": "ZKWASM",
        "quote_asset": "USDT",
        "balances": [
          {
            "asset": "ZKWASM",
            "free": 1000.0,
            "locked": 500.0,
            "total": 1500.0
          },
          {
            "asset": "USDT",
            "free": 5000.0,
            "locked": 2000.0,
            "total": 7000.0
          }
        ]
      },
      "market_depth": {
        "current_price": 0.015,
        "best_bid": 0.0149,
        "best_ask": 0.0151,
        "depth_ranges": [
          {
            "percentage": 0.5,
            "bid_depth": 1500.50,
            "ask_depth": 1600.25
          }
        ]
      },
      "order_depth": {
        "total_orders": 50,
        "buy_orders": {
          "count": 25,
          "price_min": 0.014,
          "price_max": 0.0149,
          "total_qty": 50000.0,
          "total_value": 750.0
        },
        "sell_orders": {
          "count": 25,
          "price_min": 0.0151,
          "price_max": 0.016,
          "total_qty": 45000.0,
          "total_value": 700.0
        },
        "total_unfilled_value": 1450.0
      },
      "snapshots_info": {
        "total_count": 100,
        "latest": {
          "datetime": "2025-11-12 13:40:00 UTC",
          "iteration": 50,
          "mid_price": 0.015,
          "assets": [...],
          "total_value": 7500.0
        }
      },
      "pnl_summary": {
        "period_start": "2025-11-12T10:00:00Z",
        "period_end": "2025-11-12T13:40:00Z",
        "duration_seconds": 13200,
        "starting_value": 7000.0,
        "ending_value": 7500.0,
        "absolute_change": 500.0,
        "percentage_change": 7.142
      }
    }
  ]
}
```

## Dashboard Features

The React dashboard displays:

### 1. **Account Balance**
- Shows free, locked, and total balances for each asset
- Color-coded asset display

### 2. **Market Depth**
- Current market price, best bid, and best ask
- Depth analysis at various price ranges (±0.5%, ±1%, ±2%, ±5%, ±10%)
- Interactive table view

### 3. **My Order Depth**
- Summary of your open orders
- Separate statistics for buy and sell orders
- Price ranges, quantities, and total values
- No individual order details (just aggregated statistics)

### 4. **Snapshots History**
- Total number of snapshots captured
- Latest snapshot details including iteration number and mid-price

### 5. **Profit & Loss Summary**
- Time period analysis
- Starting and ending values
- Absolute and percentage changes
- Color-coded gains (green) and losses (red)

## Configuration

### Backend Port
Default port is `3000`. Change it with the `--port` flag:
```bash
./target/release/mexc-grid-trader report --port 8080
```

### Frontend API URL
If your backend runs on a different port or host, set the environment variable:
```bash
# In frontend/.env
REACT_APP_API_URL=http://localhost:8080
```

Then restart the frontend:
```bash
npm start
```

## Deployment

### Backend
The backend can be deployed anywhere that supports Linux binaries:
```bash
# Build for release
cargo build --release

# Copy binary to server
scp target/release/mexc-grid-trader user@server:/opt/trading-bot/

# Run on server (use systemd or similar for production)
/opt/trading-bot/mexc-grid-trader report --port 3000
```

### Frontend
Build and deploy the static files:
```bash
cd frontend
npm run build

# Deploy the build/ directory to any static hosting
# Examples: Netlify, Vercel, AWS S3, Nginx, etc.
```

For production, you may want to use a reverse proxy (nginx/caddy) to:
- Serve the frontend
- Proxy API requests to the backend
- Enable HTTPS

Example nginx config:
```nginx
server {
    listen 80;
    server_name your-domain.com;

    # Serve React frontend
    location / {
        root /var/www/trading-dashboard;
        try_files $uri $uri/ /index.html;
    }

    # Proxy API requests to backend
    location /api/ {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    location /health {
        proxy_pass http://localhost:3000;
    }
}
```

## Troubleshooting

### Backend Issues
- **Port already in use**: Change the port with `--port` flag
- **Config file not found**: Specify full path with `--config`
- **API errors**: Check if exchanges are configured correctly in config.json

### Frontend Issues
- **Cannot connect to backend**: Check if backend is running and CORS is enabled
- **Port conflict**: React dev server will use next available port automatically
- **Build errors**: Run `npm install` to ensure all dependencies are installed

## Development

### Adding New Metrics
1. Update the backend API response structure in `src/api_server.rs`
2. Add corresponding TypeScript interfaces in `frontend/src/App.tsx`
3. Update the UI components to display new metrics

### Styling
All styles are in `frontend/src/App.css`. The design uses:
- Gradient backgrounds
- Card-based layout
- Responsive grid system
- Color-coded data visualization

## Notes

- The dashboard auto-refreshes every 30 seconds
- The backend caches data and updates every 30 seconds
- All timestamps are in UTC
- The API is stateless and can be scaled horizontally if needed
