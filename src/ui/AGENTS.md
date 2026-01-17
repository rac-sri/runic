# UI MODULE

**Purpose:** All TUI rendering and view logic for ratatui-based interface

## STRUCTURE
```
src/ui/
├── mod.rs            # Main draw() dispatches to view-specific draw functions
├── home.rs           # Home screen rendering
├── interact.rs       # Contract interaction UI (deployments, functions)
├── scripts.rs        # Script execution UI
├── config.rs         # Configuration UI (networks, wallets)
└── components/       # Reusable UI components
    ├── mod.rs        # Exports TextInput, SelectableList
    ├── input.rs      # TextInput component for user input
    └── list.rs       # SelectableList component for navigation
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Main draw loop | `mod.rs::draw()` | Routes to view-specific draw functions based on `app.view` |
| Home screen | `home.rs::draw()` | Shows welcome message, navigation hints |
| Contract interaction | `interact.rs` | Displays deployments list, function selectors, parameter inputs |
| Script execution | `scripts.rs::draw()` | Shows scripts list, execution status, output |
| Configuration | `config.rs` | Renders network and wallet configuration |
| Reusable components | `components/` | TextInput (input.rs), SelectableList (list.rs) |

## ANTI-PATTERNS (COMPLEXITY HOTSPOTS)
- `config.rs::draw_networks()` (76 lines) - nested UI rendering logic, could be extracted
- `config.rs::draw_wallets()` (51 lines) - wallet rendering with nested validation
- `interact.rs::draw_functions()` (53 lines) - function display with nested logic
- `interact.rs::draw_deployments_list()` (52 lines) - deployment list rendering

## NOTES
- Uses ratatui framework for terminal UI
- All draw functions take `&mut Frame` and `&App` as parameters
- Components use Block, Borders, Paragraph from ratatui::widgets
