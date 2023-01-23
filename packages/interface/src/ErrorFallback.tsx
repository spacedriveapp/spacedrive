import { captureException } from '@sentry/browser';
import { FallbackProps } from 'react-error-boundary';
import { Button } from '@sd/ui';

export function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
	const onClick = () => {
		captureException(error);
		resetErrorBoundary();
	};

	return (
		<div
			data-tauri-drag-region
			role="alert"
			className="flex flex-col items-center justify-center w-screen h-screen p-4 border rounded-lg border-app-divider bg-app"
		>
			<p className="m-3 text-sm font-bold text-ink-faint">APP CRASHED</p>
			<h1 className="text-2xl font-bold text-ink">We're past the event horizon...</h1>
			<pre className="m-2 text-ink">Error: {error.message}</pre>
			<div className="flex flex-row space-x-2 text-ink">
				<Button variant="accent" className="mt-2" onClick={resetErrorBoundary}>
					Reload
				</Button>
				<Button variant="gray" className="mt-2" onClick={onClick}>
					Send report
				</Button>
			</div>
		</div>
	);
}
