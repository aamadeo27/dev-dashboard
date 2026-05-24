# State management: Zustand + React Query (TanStack Query)

- **Zustand** for client-only UI state (modals open, selected project, scroll positions, toast queue, draft text in input boxes).
- **TanStack Query** for everything that originates in Rust (projects list, git status, sequences, run history, usage snapshot). Cache + invalidation handles the polling cases cleanly.
- **Live run events**: a dedicated Zustand store per active run keyed by `run_id`, populated from Tauri event subscriptions. Not in TanStack Query — these are push, not pull.
- **Rejected**: Redux Toolkit (overkill, more boilerplate than the project warrants); Context-only (re-render storms in run view).
