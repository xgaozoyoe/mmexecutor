# Exchange Tabs Feature

## 🎯 Overview

The dashboard now supports **tab navigation** for switching between multiple exchanges!

## ✨ Features

### Visual Tabs
- Clean, gradient-styled tabs for each exchange
- Active tab is highlighted with the primary color scheme
- Smooth hover effects and transitions
- Responsive design works on both desktop and mobile

### Behavior
- **Auto-hide**: Tabs only appear when you have 2+ exchanges configured
- **Default selection**: First exchange is selected by default
- **Instant switching**: Click any tab to instantly switch views
- **Independent data**: Each tab shows data for one exchange only

## 🎨 Design

### Desktop View
```
┌─────────────────────────────────────────────┐
│         🤖 Trading Bot Dashboard            │
│         Last Update: ...                     │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│  [  mexc  ] [ gate.io ] [ kucoin ]          │  ← Tabs
└─────────────────────────────────────────────┘
       ↑
   Active tab (purple gradient)

┌─────────────────────────────────────────────┐
│  📊 mexc                                     │
│  ┌───────────────────────────────────────┐  │
│  │  Account Balance                       │  │
│  │  ...                                   │  │
│  └───────────────────────────────────────┘  │
│  ...                                         │
└─────────────────────────────────────────────┘
```

### Mobile View
Tabs stack vertically:
```
┌───────────────┐
│    mexc       │ ← Active
├───────────────┤
│   gate.io     │
├───────────────┤
│   kucoin      │
└───────────────┘
```

## 🎨 Color Scheme

- **Inactive tabs**: Light gray gradient (#f5f7fa → #c3cfe2)
- **Active tab**: Purple gradient (#667eea → #764ba2)
- **Hover effect**: Slight lift with shadow
- **Text**: Dark for inactive, white for active

## 📱 Responsive Design

### Desktop (> 768px)
- Tabs displayed horizontally
- Equal width distribution with flex
- Minimum width: 120px per tab

### Mobile (≤ 768px)
- Tabs stack vertically
- Full width tabs
- Easier touch targets

## 🔧 Technical Details

### State Management
```typescript
const [activeTab, setActiveTab] = useState<number>(0);
```
- Uses React state to track active tab index
- Index 0 = first exchange
- Click handler updates the state

### Conditional Rendering
```typescript
style={{ display: activeTab === index ? 'block' : 'none' }}
```
- All exchanges are rendered but hidden
- Only active exchange is visible
- Preserves component state when switching

### Auto-hide Logic
```typescript
{reportData && reportData.exchanges.length > 1 && (
  <div className="tabs-container">...</div>
)}
```
- Tabs only appear if 2+ exchanges exist
- Single exchange displays directly (no tabs)

## 💡 Use Cases

### Use Case 1: Multi-Exchange Monitoring
```
Your config.json has 3 exchanges:
- MEXC
- Gate.io
- KuCoin

Dashboard shows tabs for easy switching between them!
```

### Use Case 2: Single Exchange
```
Your config.json has 1 exchange:
- Gate.io

Dashboard shows data directly (no tabs needed)
```

### Use Case 3: Comparing Exchanges
```
1. Click "MEXC" tab → See MEXC performance
2. Click "Gate.io" tab → See Gate.io performance
3. Compare average trading prices
4. Compare PnL summaries
```

## 🎯 Benefits

✅ **Clean UI**: No scrolling through multiple exchanges
✅ **Fast Switching**: Instant tab changes
✅ **Space Efficient**: One exchange visible at a time
✅ **Mobile Friendly**: Touch-optimized tabs
✅ **Clear Labels**: Exchange names clearly displayed
✅ **Visual Feedback**: Active tab is clearly highlighted

## 🚀 How to Use

1. **Start the report server:**
   ```bash
   ./target/release/mexc-grid-trader report
   ```

2. **Start the frontend:**
   ```bash
   cd frontend
   npm start
   ```

3. **Click on tabs** to switch between exchanges!

## 📊 Example Scenario

### Before (Without Tabs)
```
Scroll down...
┌──────────────┐
│ MEXC Data    │
│ (lots of     │
│  content)    │
└──────────────┘
Scroll more...
┌──────────────┐
│ Gate.io Data │
│ (lots of     │
│  content)    │
└──────────────┘
Scroll more...
┌──────────────┐
│ KuCoin Data  │
│ (lots of     │
│  content)    │
└──────────────┘
```
❌ Long scrolling required

### After (With Tabs)
```
[MEXC] [Gate.io] [KuCoin]  ← Click to switch
┌──────────────┐
│ Active       │
│ Exchange     │
│ Data Only    │
└──────────────┘
```
✅ Clean, organized, easy to navigate!

## 🎨 Customization

### Change Tab Colors
Edit `frontend/src/App.css`:

```css
.tab.active {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  /* Change these colors to your preference */
}
```

### Change Tab Size
```css
.tab {
  padding: 15px 25px;  /* Vertical, Horizontal */
  font-size: 1.1em;    /* Text size */
}
```

## 🔮 Future Enhancements

Possible improvements:
- [ ] Remember last selected tab (localStorage)
- [ ] Keyboard shortcuts (← → arrows)
- [ ] Tab badges showing alerts/status
- [ ] Drag-and-drop tab reordering
- [ ] Tab close buttons (hide exchanges)
- [ ] Comparison view (show 2 tabs side-by-side)

## 📚 Files Modified

1. **`frontend/src/App.tsx`**:
   - Added `activeTab` state
   - Added tab navigation rendering
   - Added conditional display logic

2. **`frontend/src/App.css`**:
   - Added `.tabs-container` styles
   - Added `.tabs` flex layout
   - Added `.tab` button styles
   - Added `.tab.active` active state
   - Added responsive media queries

## 🎉 Summary

The tabs feature makes managing multiple exchanges much easier:
- **Click a tab** → Switch exchange
- **Clean interface** → One exchange at a time
- **Responsive design** → Works everywhere
- **Auto-hides** → Only shows when needed

Enjoy the improved navigation! 🚀
