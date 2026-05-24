# Async and cancellation

- Every long-running task (run session, git poller, usage probe) holds a `CancellationToken` (tokio-util). Stop = cancel token, then `child.kill()` for processes.
- Window close: graceful shutdown sends cancel to all tokens, waits up to 2 seconds, then aborts.
