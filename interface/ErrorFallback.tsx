import { captureException } from '@sentry/browser';
import { FallbackProps } from 'react-error-boundary';
import { useDebugState } from '@sd/client';
import { Button } from '@sd/ui';

export default ({ error, resetErrorBoundary }: FallbackProps) => (
	<ErrorPage
		message={error.message}
		sendReportBtn={() => {
			captureException(error);
			resetErrorBoundary();
		}}
		reloadBtn={resetErrorBoundary}
	/>
);

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
			className="flex h-screen w-screen flex-col items-center justify-center rounded-lg border border-app-divider bg-app p-4"
		>
			<p className="m-3 text-sm font-bold text-ink-faint">APP CRASHED</p>
			<h1 className="text-2xl font-bold text-ink">We're past the event horizon...</h1>
			<pre className="m-2 text-ink">Error: {message}</pre>
			{debug.enabled && (
				<pre className="m-2 text-sm text-ink-dull">
					Check the console (CMD/CTRL + OPTION + i) for stack trace.
				</pre>
			)}
			<div className="flex flex-row space-x-2 text-ink">
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
				{message === 'failed to initialize config' && (
					<div className="flex flex-col items-center pt-12">
						<p className="text-center">
							We detected you may have created your library with an older version of
							<br />
							Spacedrive. Please reset it to continue using the app!
							<br />
							YOU WILL LOSE ANY EXISTING SPACEDRIVE DATA!
						</p>
						<Button
							variant="colored"
							className="mt-2 max-w-xs bg-red-500"
							onClick={() => {
								console.log('A'); // TODO
								// @ts-expect-error
								window.__TAURI_INVOKE__('reset_spacedrive');
							}}
						>
							Reset Library
						</Button>
					</div>
				)}
			</div>
		</div>
	);
}
