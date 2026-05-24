# Auth

**Decision**: **No authentication.** The app is local single-user (NFR-2). It binds nothing on the network. The only credential boundary is the OS user account. No login screen, no session token, no encryption-at-rest beyond OS filesystem permissions (config dir is per-user by default).

**Rationale**: adding auth here would be friction without security benefit. The threat model is "another user on the same machine reads my files" — and that is the OS's job. We do not need to re-implement it.
