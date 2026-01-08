import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

function ErrorFallback({
  error,
  onReset,
}: {
  error: Error;
  onReset: () => void;
}) {
  return (
    <div className="flex h-screen items-center justify-center bg-app p-8">
      <div className="max-w-2xl rounded-lg border border-app-line bg-app-box p-8 shadow-lg">
        <h1 className="mb-4 font-bold text-2xl text-ink">
          Something went wrong
        </h1>
        <p className="mb-4 text-ink-dull">
          The application encountered an error. Please try restarting.
        </p>
        <details className="mb-4">
          <summary className="cursor-pointer text-ink-faint text-sm hover:text-ink-dull">
            Error details
          </summary>
          <pre className="mt-2 overflow-auto rounded bg-app-darkBox p-4 text-ink-faint text-xs">
            {error.toString()}
            {error.stack}
          </pre>
        </details>
        <button
          className="rounded bg-accent px-4 py-2 text-white hover:bg-accent-deep"
          onClick={onReset}
        >
          Reload Application
        </button>
      </div>
    </div>
  );
}

/**
 * Error boundary for catching and displaying React errors.
 *
 * Must be a class component as React doesn't provide hook-based error boundaries yet.
 * The fallback UI is extracted as a functional component for cleaner code.
 */
export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("ErrorBoundary caught an error:", error, errorInfo);
  }

  handleReset = () => {
    window.location.reload();
  };

  render() {
    if (this.state.error) {
      return (
        <ErrorFallback error={this.state.error} onReset={this.handleReset} />
      );
    }

    return this.props.children;
  }
}
