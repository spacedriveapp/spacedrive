import { Button } from '@sd/ui';
import React from 'react';
import { FallbackProps } from 'react-error-boundary';

export function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
	return (
		<div
			data-tauri-drag-region
			role="alert"
			className="flex flex-col items-center justify-center w-screen h-screen p-4 border border-gray-200 rounded-lg dark:border-gray-650 bg-gray-50 dark:bg-gray-650 dark:text-white"
		>
			<p className="m-3 text-sm font-bold text-gray-400">APP CRASHED</p>
			<h1 className="text-2xl font-bold">We're past the event horizon...</h1>
			<pre className="m-2">Error: {error.message}</pre>
			<div className="flex flex-row space-x-2">
				<Button variant="primary" className="mt-2" onClick={resetErrorBoundary}>
					Reload
				</Button>
				<Button className="mt-2" onClick={resetErrorBoundary}>
					Send report
				</Button>
			</div>
		</div>
	);
}
