# IPC: Settings + system

```rust
get_settings() -> Settings
update_settings(patch: SettingsPatch) -> Settings
verify_claude_cli(path_override: Option<PathBuf>) -> CliCheck
get_usage() -> Option<UsageSnapshot>
refresh_usage() -> UsageSnapshot
open_logs_folder() -> ()
log_frontend_error(message: String, stack: Option<String>, route: Option<String>) -> ()

struct CliCheck { found: bool, resolved_path: Option<PathBuf>, version: Option<String>, error: Option<String> }
```
