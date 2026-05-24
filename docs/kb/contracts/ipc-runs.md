# IPC: Runs

```rust
launch_run(input: LaunchInput) -> Run
stop_run(run_id: String) -> ()
send_input(run_id: String, text: String) -> ()
respond_to_step_failure(run_id: String, choice: StepFailureChoice) -> ()
list_runs(project_id: String) -> Vec<Run>                // newest first, meta.json only
get_run(run_id: String, project_id: String) -> Run
load_transcript(run_id: String, project_id: String) -> Vec<RunEvent>
delete_run(run_id: String, project_id: String) -> ()    // optional, manual prune

struct LaunchInput {
    project_id: String,
    sequence_name: String,
    attached_md_path: Option<PathBuf>,
}

enum StepFailureChoice { Retry, Skip, Abort, Continue }
```
