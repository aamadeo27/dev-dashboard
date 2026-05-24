# Layering

- **Rust**: `ipc/commands.rs` is a thin shim. All real work lives in the domain modules (`projects/`, `runs/`, etc.). Commands take `tauri::State<AppState>` and forward to domain methods. No business logic in command bodies.
- **Frontend**: route components compose feature components. Feature components consume hooks. Hooks consume `ipc/commands.ts` and `ipc/events.ts`. No direct `invoke()` calls inside components.
