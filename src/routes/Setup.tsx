// S-01 Setup screen. See ui-ux-spec.md §5.1 and docs/tasks/T1.3.md.
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { updateSettings, verifyClaudeCli } from "../ipc/commands";

// ---------------------------------------------------------------------------
// OS detection
// ---------------------------------------------------------------------------

function detectOs(): "windows" | "macos" | "linux" {
  const platform = navigator.platform ?? "";
  if (platform.includes("Win")) return "windows";
  if (platform.includes("Mac")) return "macos";
  return "linux";
}

// FIX 2: Module-level constant — evaluated once, not on every render.
const OS = detectOs();

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function Setup() {
  const navigate = useNavigate();

  const [cliPath, setCliPath] = useState("");
  const [verifyState, setVerifyState] = useState<"idle" | "verifying" | "success" | "failure">(
    "idle"
  );
  const [verifyMessage, setVerifyMessage] = useState<string>("");

  // FIX 3: Derive isVerifying from verifyState — no separate boolean state needed.
  const isVerifying = verifyState === "verifying";
  const canVerify = cliPath.trim().length > 0 && !isVerifying;

  // -------------------------------------------------------------------------
  // Handlers
  // -------------------------------------------------------------------------

  async function handleBrowse() {
    // FIX 1: Pass executable filters on Windows; no filters on macOS/Linux.
    const dialogOptions =
      OS === "windows"
        ? {
            multiple: false,
            directory: false,
            filters: [{ name: "Executables", extensions: ["exe", "cmd", "bat"] }],
          }
        : { multiple: false, directory: false };
    const selected = await open(dialogOptions);
    if (typeof selected === "string") {
      setCliPath(selected);
      setVerifyState("idle");
      setVerifyMessage("");
    }
  }

  async function handleVerify() {
    if (!canVerify) return;
    setVerifyState("verifying");
    setVerifyMessage("");
    const pathCheckError = `Could not run \`${cliPath} --version\`. Check the path.`;
    try {
      // FIX 4: Pass cliPath directly — canVerify guard ensures it is non-empty.
      const result = await verifyClaudeCli(cliPath);
      if (result.found) {
        const version = result.version ?? "";
        setVerifyMessage(`Found: ${version}`.trim());
        setVerifyState("success");
        // FIX 5: Separate inner try/catch so updateSettings errors show a distinct message.
        try {
          await updateSettings({
            parent_dir: null,
            claude_cli_path: cliPath || null,
            git_poll_interval_secs: null,
            usage_poll_interval_secs: null,
            retention_days: null,
            retention_size_mb: null,
            view_mode: null,
          });
        } catch {
          setVerifyMessage("Verified, but could not save path. Check app permissions.");
          setVerifyState("failure");
        }
      } else {
        setVerifyMessage(pathCheckError);
        setVerifyState("failure");
      }
    } catch {
      setVerifyMessage(pathCheckError);
      setVerifyState("failure");
    }
  }

  function handleContinue() {
    navigate("/");
  }

  // -------------------------------------------------------------------------
  // Render helpers
  // -------------------------------------------------------------------------

  function renderInstallInstructions() {
    let command: string;
    let label: string;

    // detectOs() is called at render time here so per-test navigator.platform mocks
    // (used in OS-specific instruction tests) are respected. The module-level OS
    // constant is used only for handleBrowse where per-render cost matters.
    const renderOs = detectOs();

    if (renderOs === "windows") {
      command = "winget install Anthropic.Claude";
      label = "Windows (winget)";
    } else if (renderOs === "macos") {
      command = "brew install anthropic/claude/claude";
      label = "macOS (Homebrew)";
    } else {
      command = "npm install -g @anthropic-ai/claude-code";
      label = "Linux / npm";
    }

    return (
      <div>
        <p style={styles.instructionLabel}>{label}</p>
        <pre style={styles.codeBlock}>{command}</pre>
        {renderOs === "macos" && (
          <>
            <p style={styles.instructionOr}>or via npm:</p>
            <pre style={styles.codeBlock}>npm install -g @anthropic-ai/claude-code</pre>
          </>
        )}
      </div>
    );
  }

  function renderStatusArea() {
    // FIX 6: Show "Verifying..." text during in-flight state.
    if (verifyState === "verifying") {
      return <output style={styles.statusVerifying}>Verifying...</output>;
    }
    if (verifyState === "idle") return null;
    if (verifyState === "success") {
      return (
        <output style={styles.statusSuccess}>
          {"✓"} {verifyMessage}
        </output>
      );
    }
    // failure
    return (
      <p style={styles.statusFailure} role="alert">
        {verifyMessage}
      </p>
    );
  }

  return (
    <div style={styles.page}>
      <div style={styles.card}>
        {/* App name */}
        <p style={styles.appName}>Dev Dashboard</p>

        {/* Heading */}
        <h1 style={styles.heading}>Claude CLI not found</h1>

        {/* Body */}
        <p style={styles.body}>
          The Dev Dashboard requires the Claude CLI (<code style={styles.inlineCode}>claude</code>)
          to be available on your PATH. Install it or set a custom path below.
        </p>

        {/* Install instructions */}
        <div style={styles.instructionsSection}>{renderInstallInstructions()}</div>

        {/* Divider */}
        <div style={styles.dividerRow}>
          <div style={styles.dividerLine} />
          <span style={styles.dividerLabel}>Or set a custom path</span>
          <div style={styles.dividerLine} />
        </div>

        {/* Path input */}
        <div style={styles.formField}>
          <label htmlFor="cli-path" style={styles.inputLabel}>
            Claude CLI path
          </label>
          <input
            id="cli-path"
            type="text"
            value={cliPath}
            onChange={(e) => {
              setCliPath(e.target.value);
              setVerifyState("idle");
              setVerifyMessage("");
            }}
            placeholder="/usr/local/bin/claude"
            disabled={isVerifying}
            style={isVerifying ? styles.inputDisabledFull : styles.input}
            aria-label="Claude CLI path"
          />
        </div>

        {/* Button row */}
        <div style={styles.buttonRow}>
          <button
            type="button"
            style={isVerifying ? styles.secondaryDisabledFull : styles.secondaryButton}
            onClick={handleBrowse}
            disabled={isVerifying}
            aria-label="Browse for CLI path"
          >
            Browse...
          </button>

          {verifyState === "success" ? (
            <button
              type="button"
              style={styles.primaryButton}
              onClick={handleContinue}
              aria-label="Continue to Dashboard"
            >
              Continue {"→"}
            </button>
          ) : (
            <button
              type="button"
              style={canVerify ? styles.primaryButton : styles.primaryDisabledFull}
              onClick={handleVerify}
              disabled={!canVerify}
              aria-label="Verify and continue"
            >
              {isVerifying ? "Verifying..." : "Verify & Continue"}
            </button>
          )}
        </div>

        {/* Status area */}
        {renderStatusArea()}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Inline styles using CSS variables
// ---------------------------------------------------------------------------

const styles: Record<string, React.CSSProperties> = {
  page: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    minHeight: "100vh",
    background: "var(--bg-base)",
    padding: "var(--space-6)",
    boxSizing: "border-box" as const,
  },
  card: {
    background: "var(--bg-surface)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-lg)",
    padding: "var(--space-8)",
    width: "100%",
    maxWidth: "520px",
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-5)",
    color: "var(--text-primary)",
  },
  appName: {
    fontSize: "var(--font-size-xs)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    color: "var(--text-disabled)",
    textTransform: "uppercase" as const,
    letterSpacing: "0.08em",
    margin: 0,
  },
  heading: {
    fontSize: "var(--font-size-xl)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    color: "var(--text-primary)",
    margin: 0,
  },
  body: {
    fontSize: "var(--font-size-sm)",
    color: "var(--text-secondary)",
    lineHeight: 1.6,
    margin: 0,
  },
  inlineCode: {
    fontFamily: "var(--font-mono, monospace)",
    fontSize: "var(--font-size-sm)",
    background: "var(--bg-elevated)",
    padding: "0.1em 0.4em",
    borderRadius: "var(--radius-sm)",
    color: "var(--text-primary)",
  },
  instructionsSection: {
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-2)",
  },
  instructionLabel: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-disabled)",
    margin: "0 0 var(--space-1) 0",
  },
  instructionOr: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-disabled)",
    margin: "var(--space-2) 0 var(--space-1) 0",
  },
  codeBlock: {
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
    padding: "var(--space-3) var(--space-4)",
    fontFamily: "var(--font-mono, monospace)",
    fontSize: "var(--font-size-sm)",
    color: "var(--text-primary)",
    overflowX: "auto" as const,
    margin: 0,
    whiteSpace: "pre" as const,
  },
  dividerRow: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-3)",
  },
  dividerLine: {
    flex: 1,
    height: "1px",
    background: "var(--border-subtle)",
  },
  dividerLabel: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-disabled)",
    whiteSpace: "nowrap" as const,
  },
  formField: {
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-1)",
  },
  inputLabel: {
    fontSize: "var(--font-size-sm)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    color: "var(--text-primary)",
  },
  input: {
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
    color: "var(--text-primary)",
    fontSize: "var(--font-size-sm)",
    padding: "var(--space-2) var(--space-3)",
    width: "100%",
    boxSizing: "border-box" as const,
    outline: "none",
    transition: "border-color var(--duration-fast) var(--easing-out)",
  },
  buttonRow: {
    display: "flex",
    gap: "var(--space-3)",
    justifyContent: "flex-end",
  },
  secondaryButton: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-2)",
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
    color: "var(--text-secondary)",
    fontSize: "var(--font-size-sm)",
    padding: "var(--space-2) var(--space-4)",
    cursor: "pointer",
    transition:
      "background var(--duration-fast) var(--easing-out), color var(--duration-fast) var(--easing-out)",
  },
  primaryButton: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-2)",
    background: "var(--primary)",
    border: "1px solid var(--primary)",
    borderRadius: "var(--radius-md)",
    color: "var(--text-on-primary)",
    fontSize: "var(--font-size-sm)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    padding: "var(--space-2) var(--space-5)",
    cursor: "pointer",
    transition: "background var(--duration-fast) var(--easing-out)",
  },
  // Combined status styles (hoisted to avoid per-render allocations)
  statusVerifying: {
    fontSize: "var(--font-size-sm)",
    margin: 0,
    lineHeight: 1.5,
    color: "var(--text-secondary)",
  },
  statusSuccess: {
    fontSize: "var(--font-size-sm)",
    margin: 0,
    lineHeight: 1.5,
    color: "var(--success)",
  },
  statusFailure: {
    fontSize: "var(--font-size-sm)",
    margin: 0,
    lineHeight: 1.5,
    color: "var(--error)",
  },
  inputDisabledFull: {
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
    color: "var(--text-primary)",
    fontSize: "var(--font-size-sm)",
    padding: "var(--space-2) var(--space-3)",
    width: "100%",
    boxSizing: "border-box" as const,
    outline: "none",
    transition: "border-color var(--duration-fast) var(--easing-out)",
    opacity: 0.5,
    cursor: "not-allowed",
  },
  secondaryDisabledFull: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-2)",
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
    color: "var(--text-secondary)",
    fontSize: "var(--font-size-sm)",
    padding: "var(--space-2) var(--space-4)",
    cursor: "not-allowed",
    transition:
      "background var(--duration-fast) var(--easing-out), color var(--duration-fast) var(--easing-out)",
    opacity: 0.5,
  },
  primaryDisabledFull: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-2)",
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
    color: "var(--text-disabled)",
    fontSize: "var(--font-size-sm)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    padding: "var(--space-2) var(--space-5)",
    cursor: "not-allowed",
    transition: "background var(--duration-fast) var(--easing-out)",
  },
};
