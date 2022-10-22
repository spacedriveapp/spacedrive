import { rspc, usePlatform } from '@sd/client';
import { Button } from '@sd/ui';
import { FallbackProps } from 'react-error-boundary';

import { guessOperatingSystem } from './hooks/useOperatingSystem';

export function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
	const platform = usePlatform();
	const version = 'unknown'; // TODO: Embed the version into the frontend via ENV var when compiled so we can use it here.

	return (
		<div
			data-tauri-drag-region
			role="alert"
			className="flex flex-col items-center justify-center w-screen h-screen p-4 border rounded-lg border-app-divider bg-app"
		>
			<p className="m-3 text-sm font-bold text-ink-faint">APP CRASHED</p>
			<h1 className="text-2xl font-bold">We're past the event horizon...</h1>
			<pre className="m-2">Error: {error.message}</pre>
			<div className="flex flex-row space-x-2">
				<Button variant="accent" className="mt-2" onClick={resetErrorBoundary}>
					Reload
				</Button>
				<Button
					variant="gray"
					className="mt-2"
					onClick={() => {
						platform.openLink(
							`https://github.com/spacedriveapp/spacedrive/issues/new?assignees=&labels=kind%2Fbug%2Cstatus%2Fneeds-triage&template=bug_report.yml&logs=${encodeURIComponent(
								error.toString()
							)}&info=${encodeURIComponent(
								`App version ${version} running on ${guessOperatingSystem() || 'unknown'}`
							)}`
						);

						resetErrorBoundary();
					}}
				>
					Send report
				</Button>
			</div>
		</div>
	);
}
