import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
	children: ReactNode;
}

interface State {
	hasError: boolean;
	error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
	constructor(props: Props) {
		super(props);
		this.state = { hasError: false, error: null };
	}

	static getDerivedStateFromError(error: Error): State {
		return { hasError: true, error };
	}

	componentDidCatch(error: Error, errorInfo: ErrorInfo) {
		console.error('ErrorBoundary caught an error:', error, errorInfo);
	}

	render() {
		if (this.state.hasError) {
			return (
				<div className="flex h-screen items-center justify-center bg-app p-8">
					<div className="max-w-2xl rounded-lg border border-app-line bg-app-box p-8 shadow-lg">
						<h1 className="mb-4 text-2xl font-bold text-ink">
							Something went wrong
						</h1>
						<p className="mb-4 text-ink-dull">
							The application encountered an error. Please try restarting.
						</p>
						{this.state.error && (
							<details className="mb-4">
								<summary className="cursor-pointer text-sm text-ink-faint hover:text-ink-dull">
									Error details
								</summary>
								<pre className="mt-2 overflow-auto rounded bg-app-darkBox p-4 text-xs text-ink-faint">
									{this.state.error.toString()}
									{this.state.error.stack}
								</pre>
							</details>
						)}
						<button
							onClick={() => window.location.reload()}
							className="rounded bg-accent px-4 py-2 text-white hover:bg-accent-deep"
						>
							Reload Application
						</button>
					</div>
				</div>
			);
		}

		return this.props.children;
	}
}
