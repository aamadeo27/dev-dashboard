# Token usage hook

`log-usage.sh` captures real per-turn token usage from Claude Code's transcript and appends it to `DevTeam.log`. Pair it with the orchestrator's `[Usage checkpoint]` lines to compute cost per Task / per fix pass / per Epic.

## Requirements

- `bash`
- `jq`
- `tac` (GNU coreutils; on macOS use `gtac` from coreutils via Homebrew, or replace with `tail -r` in the script)

## Install

Add to `.claude/settings.json` (project-level) or `~/.claude/settings.json` (user-level):

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": ".*",
        "hooks": [
          { "type": "command", "command": "bash .claude/hooks/log-usage.sh" }
        ]
      }
    ],
    "SubagentStop": [
      {
        "matcher": ".*",
        "hooks": [
          { "type": "command", "command": "bash .claude/hooks/log-usage.sh" }
        ]
      }
    ]
  }
}
```

Make the script executable: `chmod +x .claude/hooks/log-usage.sh`.

## Output format

```
[2026-05-23T18:42:11Z] [usage-hook] [Usage tokens] in=1240 out=4801 cache_read=88000 cache_create=2400 model=claude-sonnet-4-6-... source=subagent-stop
```

`Stop` lines = main thread turn end. `SubagentStop` lines = a Task subagent finished.

## How to compute cost per phase

Between two consecutive `[Usage checkpoint]` lines emitted by the orchestrator, sum all `[Usage tokens]` lines. That's the cost of the phase (Task / iteration / wave).

Example bash one-liner to extract all checkpoints + usage in a file:

```bash
grep -E '\[(Usage checkpoint|Usage tokens)\]' DevTeam.log
```

Pipe to your favourite analyzer (Python, jq, etc.) to aggregate.
