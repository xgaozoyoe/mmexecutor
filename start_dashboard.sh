#!/bin/bash

# Trading Bot Dashboard Launcher
# This script starts both the backend API server and the React frontend

set -e

echo "🚀 Starting Trading Bot Dashboard..."
echo ""

# Check if backend binary exists
if [ ! -f "target/release/mexc-grid-trader" ]; then
    echo "⚠️  Backend binary not found. Building..."
    cargo build --release
fi

# Start backend in background
echo "📊 Starting backend API server on port 3000..."
./target/release/mexc-grid-trader report --port 3000 &
BACKEND_PID=$!

# Wait a moment for backend to start
sleep 2

# Check if backend is running
if ! kill -0 $BACKEND_PID 2>/dev/null; then
    echo "❌ Failed to start backend server"
    exit 1
fi

echo "✅ Backend server started (PID: $BACKEND_PID)"
echo ""

# Check if frontend dependencies are installed
if [ ! -d "frontend/node_modules" ]; then
    echo "📦 Installing frontend dependencies..."
    cd frontend
    npm install
    cd ..
fi

# Start frontend
echo "🎨 Starting React frontend..."
cd frontend
npm start &
FRONTEND_PID=$!
cd ..

echo ""
echo "═══════════════════════════════════════════════════════"
echo "✨ Dashboard is starting!"
echo "═══════════════════════════════════════════════════════"
echo "Backend API:  http://localhost:3000/api/report"
echo "Frontend:     http://localhost:3000 (or next available port)"
echo "═══════════════════════════════════════════════════════"
echo ""
echo "Press Ctrl+C to stop all services"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "🛑 Stopping services..."
    kill $BACKEND_PID 2>/dev/null || true
    kill $FRONTEND_PID 2>/dev/null || true
    # Kill any node processes related to react-scripts
    pkill -f "react-scripts" 2>/dev/null || true
    echo "✅ All services stopped"
    exit 0
}

# Trap Ctrl+C and call cleanup
trap cleanup INT TERM

# Wait for both processes
wait
