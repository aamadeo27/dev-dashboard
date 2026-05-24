# Frontend: React + TypeScript + Vite

- **Framework**: **React 18 + TypeScript**, built with **Vite**.
- **Justification**:
  - Single developer + Sonnet coders: React has the largest training corpus, fewest surprise idioms.
  - Component library reuse: the UI spec has a clear component inventory (`ProjectCard`, `EventBlock`, etc.) — React's component model maps 1:1.
  - Streaming-heavy run view benefits from `useSyncExternalStore` + memoization patterns that are well-trodden in React.
  - TS is non-negotiable for the IPC contract surface (Rust types -> TS types via `ts-rs` or hand-mirrored).
- **Rejected**: Svelte (smaller ecosystem for the dev tools we need; coder unfamiliarity tax); Vue (no clear win over React here); SolidJS (too niche for a Sonnet coder to navigate confidently).
