import React from "react";

interface Props {
  children: React.ReactNode;
  fallback?: React.ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // Forward to Rust logging once IPC is wired (T0.6)
    // For now: console.error so it shows up in dev tools
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        this.props.fallback ?? (
          <div style={{ padding: "24px", color: "var(--error)" }}>
            <strong>Something went wrong.</strong>
            <p style={{ marginTop: "8px", fontSize: "var(--font-size-sm)", opacity: 0.7 }}>
              Check the application logs for details.
            </p>
          </div>
        )
      );
    }
    return this.props.children;
  }
}
