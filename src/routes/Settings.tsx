import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, FolderOpen, Save } from "lucide-react";
// S-07 Settings screen. See ui-ux-spec.md §5.7 and docs/tasks/T1.4.md.
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSettings } from "../hooks/useSettings";
import type { CliCheck, SettingsPatch, ViewMode } from "../ipc/bindings";
import { openLogsFolder, verifyClaudeCli } from "../ipc/commands";
import { useUiStore } from "../stores/ui";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function emptyToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function nullToEmpty(value: string | null | undefined): string {
  return value ?? "";
}

function clampInt(value: string, min: number, max: number, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  if (Number.isNaN(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

// ---------------------------------------------------------------------------
// Draft form state
// ---------------------------------------------------------------------------

interface DraftState {
  claudeCliPath: string;
  parentDir: string;
  gitPollInterval: string;
  usagePollInterval: string;
  retentionDays: string;
  retentionSizeMb: string;
  viewMode: ViewMode;
}

// ---------------------------------------------------------------------------
// Validation errors
// ---------------------------------------------------------------------------

interface ValidationErrors {
  gitPollInterval?: string;
  usagePollInterval?: string;
  retentionDays?: string;
  retentionSizeMb?: string;
}

function validate(draft: DraftState): ValidationErrors {
  const errors: ValidationErrors = {};

  const gitPoll = Number.parseInt(draft.gitPollInterval, 10);
  if (Number.isNaN(gitPoll) || gitPoll < 5 || gitPoll > 3600) {
    errors.gitPollInterval = "Must be an integer between 5 and 3600";
  }

  const usagePoll = Number.parseInt(draft.usagePollInterval, 10);
  if (Number.isNaN(usagePoll) || usagePoll < 30 || usagePoll > 3600) {
    errors.usagePollInterval = "Must be an integer between 30 and 3600";
  }

  const days = Number.parseInt(draft.retentionDays, 10);
  if (Number.isNaN(days) || days < 1 || days > 90) {
    errors.retentionDays = "Must be an integer between 1 and 90";
  }

  const sizeMb = Number.parseInt(draft.retentionSizeMb, 10);
  if (Number.isNaN(sizeMb) || sizeMb < 50 || sizeMb > 10240) {
    errors.retentionSizeMb = "Must be an integer between 50 and 10240";
  }

  return errors;
}

function hasErrors(errors: ValidationErrors): boolean {
  return Object.keys(errors).length > 0;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function Settings() {
  const navigate = useNavigate();
  const { settings, isLoading, updateSettings, isSaving } = useSettings();
  const setViewMode = useUiStore((s) => s.setViewMode);

  // Draft form state — initialised once settings load
  const [draft, setDraft] = useState<DraftState>({
    claudeCliPath: "",
    parentDir: "",
    gitPollInterval: "10",
    usagePollInterval: "60",
    retentionDays: "30",
    retentionSizeMb: "500",
    viewMode: useUiStore.getState().viewMode,
  });

  // Fix 3: useRef instead of useState for initialized — one-time guard, not display state
  const initializedRef = useRef(false);
  const [savedConfirmation, setSavedConfirmation] = useState(false);
  const [openingLogs, setOpeningLogs] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Fix 5: CLI verify state
  const [cliVerifyResult, setCliVerifyResult] = useState<CliCheck | null>(null);
  const [verifying, setVerifying] = useState(false);

  // Fix 1: track timer ref to clear on unmount
  const savedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Fix 1: cleanup effect — clear timer on unmount
  useEffect(
    () => () => {
      if (savedTimerRef.current) clearTimeout(savedTimerRef.current);
    },
    []
  );

  // Initialize draft from loaded settings (runs once when data arrives)
  // Fix 3: uses initializedRef instead of initialized state; reads viewMode once via getState()
  useEffect(() => {
    if (settings && !initializedRef.current) {
      initializedRef.current = true;
      setDraft({
        claudeCliPath: nullToEmpty(settings.claude_cli_path),
        parentDir: nullToEmpty(settings.parent_dir),
        gitPollInterval: String(settings.git_poll_interval_secs),
        usagePollInterval: String(settings.usage_poll_interval_secs),
        retentionDays: String(settings.retention_days),
        retentionSizeMb: String(settings.retention_size_mb),
        // viewMode comes from Zustand store — read once, not reactive
        viewMode: useUiStore.getState().viewMode,
      });
    }
  }, [settings]);

  // Fix 7: isDirty via useMemo
  const isDirty = useMemo(() => {
    if (!settings || !initializedRef.current) return false;
    return (
      emptyToNull(draft.claudeCliPath) !== settings.claude_cli_path ||
      emptyToNull(draft.parentDir) !== settings.parent_dir ||
      Number.parseInt(draft.gitPollInterval, 10) !== settings.git_poll_interval_secs ||
      Number.parseInt(draft.usagePollInterval, 10) !== settings.usage_poll_interval_secs ||
      Number.parseInt(draft.retentionDays, 10) !== settings.retention_days ||
      Number.parseInt(draft.retentionSizeMb, 10) !== settings.retention_size_mb ||
      draft.viewMode !== settings.view_mode
    );
  }, [settings, draft]);

  // Fix 7: errors via useMemo
  const errors = useMemo(() => validate(draft), [draft]);

  const canSave = !hasErrors(errors) && !isSaving;

  // Warn on browser/Tauri window close when dirty
  useEffect(() => {
    if (!isDirty) return;
    function handleBeforeUnload(e: BeforeUnloadEvent) {
      e.preventDefault();
    }
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, [isDirty]);

  // Field updaters
  function setField<K extends keyof DraftState>(key: K, value: DraftState[K]) {
    setDraft((prev) => ({ ...prev, [key]: value }));
  }

  function handleViewModeToggle(mode: ViewMode) {
    setField("viewMode", mode);
    // Optimistic sync to Zustand store immediately
    setViewMode(mode);
  }

  // Fix 2: Save handler rolls back Zustand viewMode on error
  async function handleSave() {
    if (!canSave) return;
    setSaveError(null);
    // Capture previous viewMode before the patch so we can roll back on error
    const previousViewMode = useUiStore.getState().viewMode;

    const patch: SettingsPatch = {
      claude_cli_path: emptyToNull(draft.claudeCliPath),
      parent_dir: emptyToNull(draft.parentDir),
      git_poll_interval_secs: clampInt(draft.gitPollInterval, 5, 3600, 10),
      usage_poll_interval_secs: clampInt(draft.usagePollInterval, 30, 3600, 60),
      retention_days: clampInt(draft.retentionDays, 1, 90, 30),
      retention_size_mb: clampInt(draft.retentionSizeMb, 50, 10240, 500),
      view_mode: draft.viewMode,
    };

    try {
      await updateSettings(patch);
      setSavedConfirmation(true);
      // Fix 1: clear any existing timer before setting new one
      if (savedTimerRef.current) clearTimeout(savedTimerRef.current);
      savedTimerRef.current = setTimeout(() => setSavedConfirmation(false), 2000);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setSaveError(msg);
      // Fix 2: roll back Zustand store to previous viewMode since save failed
      setViewMode(previousViewMode);
    }
  }

  // Fix 4: Browse for Claude CLI path
  async function handleBrowseCli() {
    const selected = await open({
      multiple: false,
      directory: false,
    });
    if (typeof selected === "string") {
      setField("claudeCliPath", selected);
      setCliVerifyResult(null); // clear any previous verify result
    }
  }

  // Fix 5: Verify Claude CLI
  async function handleVerifyCli() {
    setVerifying(true);
    setCliVerifyResult(null);
    try {
      const pathOverride = emptyToNull(draft.claudeCliPath) ?? undefined;
      const result = await verifyClaudeCli(pathOverride);
      setCliVerifyResult(result);
    } finally {
      setVerifying(false);
    }
  }

  // Open logs folder handler
  async function handleOpenLogs() {
    setOpeningLogs(true);
    try {
      await openLogsFolder();
    } finally {
      setOpeningLogs(false);
    }
  }

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  if (isLoading) {
    return (
      <div style={styles.page}>
        <p style={{ color: "var(--text-secondary)" }}>Loading settings…</p>
      </div>
    );
  }

  return (
    <div style={styles.page}>
      {/* Header */}
      <div style={styles.header}>
        <button
          type="button"
          style={styles.backButton}
          onClick={() => {
            if (isDirty) {
              const ok = window.confirm("You have unsaved changes. Leave without saving?");
              if (!ok) return;
            }
            navigate(-1);
          }}
          aria-label="Go back"
        >
          <ArrowLeft size={16} />
          <span>Back</span>
        </button>
        <h1 style={styles.title}>Settings</h1>
      </div>

      {/* Form */}
      <form
        style={styles.form}
        onSubmit={(e) => {
          e.preventDefault();
          handleSave();
        }}
      >
        {/* ---- Fix 5: Claude CLI section (separate fieldset) ---- */}
        <fieldset style={styles.fieldset}>
          <legend style={styles.legend}>Claude CLI</legend>

          <FormField
            fieldId="setting-claude-cli-path"
            label="Claude CLI path"
            hint="Leave blank to use PATH"
          >
            {/* Fix 4: flex row with Browse button */}
            <div style={styles.inputRow}>
              <input
                id="setting-claude-cli-path"
                type="text"
                value={draft.claudeCliPath}
                onChange={(e) => {
                  setField("claudeCliPath", e.target.value);
                  setCliVerifyResult(null);
                }}
                placeholder="e.g. /usr/local/bin/claude"
                style={{ ...styles.input, flex: 1 }}
              />
              <button type="button" style={styles.secondaryButton} onClick={handleBrowseCli}>
                Browse...
              </button>
            </div>
          </FormField>

          {/* Fix 5: Verify button + result display */}
          <div style={styles.verifyRow}>
            <button
              type="button"
              style={styles.secondaryButton}
              onClick={handleVerifyCli}
              disabled={verifying}
            >
              {verifying ? "Verifying..." : "Verify"}
            </button>
          </div>

          {cliVerifyResult !== null && (
            <p
              style={{
                ...styles.helperText,
                color: cliVerifyResult.found ? "var(--success)" : "var(--error)",
              }}
            >
              {cliVerifyResult.found
                ? `Currently using: ${cliVerifyResult.resolved_path ?? "unknown"} ${cliVerifyResult.version ?? ""}`.trim()
                : (cliVerifyResult.error ?? "Verification failed")}
            </p>
          )}
        </fieldset>

        {/* ---- Projects section ---- */}
        <fieldset style={styles.fieldset}>
          <legend style={styles.legend}>Projects</legend>

          <FormField
            fieldId="setting-parent-dir"
            label="Projects parent directory"
            hint="Leave blank to disable"
          >
            <input
              id="setting-parent-dir"
              type="text"
              value={draft.parentDir}
              onChange={(e) => setField("parentDir", e.target.value)}
              placeholder="e.g. /home/user/projects"
              style={styles.input}
            />
          </FormField>
        </fieldset>

        {/* ---- Poll intervals ---- */}
        <fieldset style={styles.fieldset}>
          <legend style={styles.legend}>Polling</legend>

          <FormField
            fieldId="setting-git-poll"
            label="Git poll interval (seconds)"
            hint="5 – 3600"
            error={errors.gitPollInterval}
          >
            <input
              id="setting-git-poll"
              type="number"
              value={draft.gitPollInterval}
              min={5}
              max={3600}
              step={1}
              onChange={(e) => setField("gitPollInterval", e.target.value)}
              style={
                errors.gitPollInterval ? { ...styles.input, ...styles.inputError } : styles.input
              }
            />
          </FormField>

          <FormField
            fieldId="setting-usage-poll"
            label="Usage poll interval (seconds)"
            hint="30 – 3600"
            error={errors.usagePollInterval}
          >
            <input
              id="setting-usage-poll"
              type="number"
              value={draft.usagePollInterval}
              min={30}
              max={3600}
              step={1}
              onChange={(e) => setField("usagePollInterval", e.target.value)}
              style={
                errors.usagePollInterval ? { ...styles.input, ...styles.inputError } : styles.input
              }
            />
          </FormField>
        </fieldset>

        {/* ---- Retention ---- */}
        <fieldset style={styles.fieldset}>
          <legend style={styles.legend}>Log Retention</legend>

          {/* Fix 6: retention helper text */}
          <p style={styles.helperText}>
            Runs are pruned at startup and once daily. Both limits apply; whichever is exceeded
            first triggers pruning.
          </p>

          <FormField
            fieldId="setting-retention-days"
            label="Retention days"
            hint="1 – 90"
            error={errors.retentionDays}
          >
            <input
              id="setting-retention-days"
              type="number"
              value={draft.retentionDays}
              min={1}
              max={90}
              step={1}
              onChange={(e) => setField("retentionDays", e.target.value)}
              style={
                errors.retentionDays ? { ...styles.input, ...styles.inputError } : styles.input
              }
            />
          </FormField>

          <FormField
            fieldId="setting-retention-size"
            label="Retention size (MB)"
            hint="50 – 10240"
            error={errors.retentionSizeMb}
          >
            <input
              id="setting-retention-size"
              type="number"
              value={draft.retentionSizeMb}
              min={50}
              max={10240}
              step={1}
              onChange={(e) => setField("retentionSizeMb", e.target.value)}
              style={
                errors.retentionSizeMb ? { ...styles.input, ...styles.inputError } : styles.input
              }
            />
          </FormField>
        </fieldset>

        {/* ---- View mode ---- */}
        <fieldset style={styles.fieldset}>
          <legend style={styles.legend}>Display</legend>

          {/* Fix 8: proper role=group instead of FormField wrapper with no fieldId */}
          <div style={styles.formField}>
            <span style={styles.labelText}>View mode</span>
            {/* biome-ignore lint/a11y/useSemanticElements: <fieldset> would impose default border/margin breaking the inline toggle layout; role=group + aria-label is the intended pattern */}
            <div role="group" aria-label="View mode" style={styles.toggleGroup}>
              <button
                type="button"
                style={
                  draft.viewMode === "Grid"
                    ? { ...styles.toggleButton, ...styles.toggleButtonActive }
                    : styles.toggleButton
                }
                onClick={() => handleViewModeToggle("Grid")}
                aria-pressed={draft.viewMode === "Grid"}
              >
                Grid
              </button>
              <button
                type="button"
                style={
                  draft.viewMode === "List"
                    ? { ...styles.toggleButton, ...styles.toggleButtonActive }
                    : styles.toggleButton
                }
                onClick={() => handleViewModeToggle("List")}
                aria-pressed={draft.viewMode === "List"}
              >
                List
              </button>
            </div>
          </div>
        </fieldset>

        {/* ---- Actions ---- */}
        <div style={styles.actions}>
          {/* Open logs folder */}
          <button
            type="button"
            style={styles.secondaryButton}
            onClick={handleOpenLogs}
            disabled={openingLogs}
            aria-label="Open logs folder"
          >
            <FolderOpen size={16} />
            <span>{openingLogs ? "Opening…" : "Open logs folder"}</span>
          </button>

          <div style={styles.saveRow}>
            {/* Save error */}
            {saveError && (
              <span style={styles.saveErrorText} role="alert">
                {saveError}
              </span>
            )}

            {/* Saved confirmation */}
            {savedConfirmation && <output style={styles.savedConfirmation}>Saved!</output>}

            {/* Save button */}
            <button
              type="submit"
              style={
                canSave
                  ? styles.primaryButton
                  : { ...styles.primaryButton, ...styles.primaryButtonDisabled }
              }
              disabled={!canSave}
              aria-label="Save settings"
            >
              <Save size={16} />
              <span>{isSaving ? "Saving…" : "Save"}</span>
            </button>
          </div>
        </div>
      </form>
    </div>
  );
}

// ---------------------------------------------------------------------------
// FormField helper
// ---------------------------------------------------------------------------

interface FormFieldProps {
  label: string;
  hint?: string;
  error?: string;
  fieldId?: string;
  children: React.ReactNode;
}

function FormField({ label, hint, error, fieldId, children }: FormFieldProps) {
  return (
    <div style={styles.formField}>
      <label htmlFor={fieldId} style={styles.labelRow}>
        <span style={styles.labelText}>{label}</span>
        {hint && <span style={styles.hint}>{hint}</span>}
      </label>
      {children}
      {error && (
        <span style={styles.errorText} role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Inline styles using CSS variables
// ---------------------------------------------------------------------------

// We use React.CSSProperties objects so TypeScript is happy and CSS variables
// from tokens.css are used exclusively — no hardcoded colors.
const styles: Record<string, React.CSSProperties> = {
  page: {
    padding: "var(--space-6)",
    maxWidth: "640px",
    margin: "0 auto",
    color: "var(--text-primary)",
  },
  header: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-4)",
    marginBottom: "var(--space-6)",
  },
  backButton: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-2)",
    background: "none",
    border: "none",
    cursor: "pointer",
    color: "var(--text-secondary)",
    fontSize: "var(--font-size-sm)",
    padding: "var(--space-1) var(--space-2)",
    borderRadius: "var(--radius-md)",
    transition: "color var(--duration-fast) var(--easing-out)",
  },
  title: {
    fontSize: "var(--font-size-lg)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    margin: 0,
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-6)",
  },
  fieldset: {
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-lg)",
    padding: "var(--space-4)",
    margin: 0,
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-4)",
  },
  legend: {
    fontSize: "var(--font-size-sm)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    color: "var(--text-secondary)",
    padding: "0 var(--space-2)",
  },
  formField: {
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-1)",
  },
  labelRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "baseline",
    gap: "var(--space-2)",
  },
  labelText: {
    fontSize: "var(--font-size-sm)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
    color: "var(--text-primary)",
  },
  hint: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-disabled)",
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
  inputError: {
    borderColor: "var(--error)",
  },
  errorText: {
    fontSize: "var(--font-size-xs)",
    color: "var(--error)",
  },
  inputRow: {
    display: "flex",
    gap: "var(--space-2)",
  },
  verifyRow: {
    display: "flex",
    gap: "var(--space-2)",
  },
  helperText: {
    fontSize: "var(--font-size-xs)",
    color: "var(--text-disabled)",
    margin: 0,
  },
  toggleGroup: {
    display: "flex",
    gap: "var(--space-2)",
  },
  toggleButton: {
    background: "var(--bg-elevated)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-md)",
    color: "var(--text-secondary)",
    fontSize: "var(--font-size-sm)",
    padding: "var(--space-2) var(--space-4)",
    cursor: "pointer",
    transition:
      "background var(--duration-fast) var(--easing-out), color var(--duration-fast) var(--easing-out), border-color var(--duration-fast) var(--easing-out)",
  },
  toggleButtonActive: {
    background: "var(--primary-dim)",
    borderColor: "var(--primary)",
    color: "var(--primary)",
  },
  actions: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    gap: "var(--space-4)",
    paddingTop: "var(--space-4)",
    borderTop: "1px solid var(--border-subtle)",
  },
  saveRow: {
    display: "flex",
    alignItems: "center",
    gap: "var(--space-3)",
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
    padding: "var(--space-2) var(--space-4)",
    cursor: "pointer",
    transition: "background var(--duration-fast) var(--easing-out)",
  },
  primaryButtonDisabled: {
    background: "var(--bg-elevated)",
    borderColor: "var(--border-subtle)",
    color: "var(--text-disabled)",
    cursor: "not-allowed",
  },
  savedConfirmation: {
    fontSize: "var(--font-size-sm)",
    color: "var(--success)",
    fontWeight: "var(--font-weight-semibold)" as unknown as number,
  },
  saveErrorText: {
    fontSize: "var(--font-size-xs)",
    color: "var(--error)",
    maxWidth: "240px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
};
