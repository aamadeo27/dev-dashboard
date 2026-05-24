// RunManager — registry of active RunSession background tasks.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::Run;

/// Handle stored in the session map per active run.
///
/// Shared between the command handlers (`stop_run`, `send_input`) and the
/// background I/O task via `Arc`.
pub struct SessionHandle {
    /// Cancellation token — call `cancel()` to stop the run.
    pub cancel: CancellationToken,
    /// Child stdin for `send_input`. Set to `None` when the child closes stdin.
    pub stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    /// Mutable run state updated by the background I/O task.
    pub run: Arc<Mutex<Run>>,
    /// Sender half of the UserInput channel.  `send_input` pushes text here so
    /// the I/O loop can write a `UserInput` event to the transcript.
    pub input_tx: tokio::sync::mpsc::UnboundedSender<String>,
}

/// Manages all active `RunSession` background tasks.
///
/// The `sessions` map is held behind an `Arc<Mutex<...>>` so that
/// `Arc` clones of the map can be passed into background tasks without
/// holding a reference to the `RunManager` itself.
#[derive(Default)]
pub struct RunManager {
    pub sessions: Arc<Mutex<HashMap<String, Arc<SessionHandle>>>>,
}

impl RunManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_arc() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new()))
    }
}
