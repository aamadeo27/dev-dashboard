#!/usr/bin/env bash
#
# Hook: log Claude usage to DevTeam.log
#
# Wire as a Stop or SubagentStop hook in .claude/settings.json (see README in this folder).
# The hook receives a JSON payload on stdin with `transcript_path` and `cwd` fields.
# We read the latest assistant message's `message.usage` block and append a line:
#
#   [<ts>] [usage-hook] [Usage tokens] in=N out=N cache_read=N cache_create=N agent=<name> source=<stop|subagent-stop>
#
# Requires `jq`.

set -euo pipefail

PAYLOAD="$(cat)"

TRANSCRIPT_PATH="$(printf '%s' "$PAYLOAD" | jq -r '.transcript_path // empty')"
CWD="$(printf '%s' "$PAYLOAD" | jq -r '.cwd // empty')"
HOOK_EVENT="$(printf '%s' "$PAYLOAD" | jq -r '.hook_event_name // "unknown"')"

[[ -z "$TRANSCRIPT_PATH" ]] && exit 0
[[ ! -f "$TRANSCRIPT_PATH" ]] && exit 0
[[ -z "$CWD" ]] && CWD="$PWD"

LAST_USAGE="$(tac "$TRANSCRIPT_PATH" \
  | jq -c 'select(.type == "assistant" and (.message.usage // empty | length > 0))' \
  | head -n 1 \
  || true)"

[[ -z "$LAST_USAGE" ]] && exit 0

IN="$(printf '%s' "$LAST_USAGE" | jq -r '.message.usage.input_tokens // 0')"
OUT="$(printf '%s' "$LAST_USAGE" | jq -r '.message.usage.output_tokens // 0')"
CACHE_READ="$(printf '%s' "$LAST_USAGE" | jq -r '.message.usage.cache_read_input_tokens // 0')"
CACHE_CREATE="$(printf '%s' "$LAST_USAGE" | jq -r '.message.usage.cache_creation_input_tokens // 0')"
MODEL="$(printf '%s' "$LAST_USAGE" | jq -r '.message.model // "unknown"')"

case "$HOOK_EVENT" in
    SubagentStop) SOURCE="subagent-stop" ;;
    Stop)         SOURCE="stop" ;;
    *)            SOURCE="$HOOK_EVENT" ;;
esac

TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

printf '[%s] [usage-hook] [Usage tokens] in=%s out=%s cache_read=%s cache_create=%s model=%s source=%s\n' \
    "$TS" "$IN" "$OUT" "$CACHE_READ" "$CACHE_CREATE" "$MODEL" "$SOURCE" \
    >> "$CWD/DevTeam.log"
