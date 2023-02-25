import { captureException } from '@sentry/browser';
import { FallbackProps } from 'react-error-boundary';
import { useDebugState } from '@sd/client';
import { Button } from '@sd/ui';

export function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
	return (
		<ErrorPage
			message={error.message}
			sendReportBtn={() => {
				captureException(error);
				resetErrorBoundary();
			}}
			reloadBtn={resetErrorBoundary}
		/>
	);
}

export function ErrorPage({
	reloadBtn,
	sendReportBtn,
	message
}: {
	reloadBtn?: () => void;
	sendReportBtn?: () => void;
	message: string;
}) {
	const debug = useDebugState();

	return (
		<div
			data-tauri-drag-region
			role="alert"
			className="border-app-divider bg-app flex h-screen w-screen flex-col items-center justify-center rounded-lg border p-4"
		>
			<p className="text-ink-faint m-3 text-sm font-bold">APP CRASHED</p>
			<h1 className="text-ink text-2xl font-bold">We're past the event horizon...</h1>
			<pre className="text-ink m-2">Error: {message}</pre>
			{debug.enabled && (
				<pre className="text-ink-dull m-2 text-sm">
					Check the console (CMD/CRTL + OPTION + i) for stack trace.
				</pre>
			)}
			<div className="text-ink flex flex-row space-x-2">
				{reloadBtn && (
					<Button variant="accent" className="mt-2" onClick={reloadBtn}>
						Reload
					</Button>
				)}
				{sendReportBtn && (
					<Button variant="gray" className="mt-2" onClick={sendReportBtn}>
						Send report
					</Button>
				)}
			</div>
		</div>
	);
}
