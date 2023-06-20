import { captureException } from '@sentry/browser';
import { FallbackProps } from 'react-error-boundary';
import { useRouteError } from 'react-router';
import { useDebugState } from '@sd/client';
import { Button } from '@sd/ui';
import { useOperatingSystem, useTheme } from './hooks';

export function RouterErrorBoundary() {
	const error = useRouteError();
	return (
		<ErrorPage
			message={(error as any).toString()}
			sendReportBtn={() => {
				captureException(error);
				location.reload();
			}}
			reloadBtn={() => {
				location.reload();
			}}
		/>
	);
}

export default ({ error, resetErrorBoundary }: FallbackProps) => (
	<ErrorPage
		message={`Error: ${error.message}`}
		sendReportBtn={() => {
			captureException(error);
			resetErrorBoundary();
		}}
		reloadBtn={resetErrorBoundary}
	/>
);

// This is sketchy but these are all edge cases that will only be encountered by developers if everything works as expected so it's probs fine
const errorsThatRequireACoreReset = [
	'failed to initialize config',
	'failed to initialize library manager: failed to run library migrations',
	'failed to initialize config: We detected a Spacedrive config from a super early version of the app!',
	'failed to initialize library manager: failed to run library migrations: YourAppIsOutdated - the config file is for a newer version of the app. Please update to the latest version to load it!'
];

export function ErrorPage({
	reloadBtn,
	sendReportBtn,
	message,
	submessage
}: {
	reloadBtn?: () => void;
	sendReportBtn?: () => void;
	message: string;
	submessage?: string;
}) {
	useTheme();
	const debug = useDebugState();
	const os = useOperatingSystem();
	const isMacOS = os === 'macOS';
	const isDev = process.env.NODE_ENV === 'development';
	if (!submessage && debug.enabled)
		submessage = 'Check the console (CMD/CTRL + OPTION + i) for stack trace.';

	return (
		<div
			data-tauri-drag-region
			role="alert"
			className={
				'flex h-screen w-screen flex-col items-center justify-center border border-app-divider p-4' +
				(isMacOS ? ' rounded-lg' : '')
			}
		>
			<p className="m-3 text-sm font-bold text-ink-faint">APP CRASHED</p>
			<h1 className="text-2xl font-bold text-ink">We're past the event horizon...</h1>
			<pre className="m-2 max-w-[650px] whitespace-normal text-center text-ink">
				{message}
			</pre>
			{submessage && <pre className="m-2 text-sm text-ink-dull">{submessage}</pre>}
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
				{(errorsThatRequireACoreReset.includes(message) ||
					message.startsWith('NodeError::FailedToInitializeConfig') ||
					message.startsWith('failed to initialize library manager')) && (
					<div className="flex flex-col items-center pt-12">
						<p className="text-md max-w-[650px] text-center">
							We detected you may have created your library with an older version of
							Spacedrive. Please reset it to continue using the app!
						</p>
						<p className="mt-3 font-bold">
							{' '}
							YOU WILL LOSE ANY EXISTING SPACEDRIVE DATA!
						</p>
						<Button
							variant="colored"
							className="max-w-xs mt-4 bg-red-500 border-transparent"
							onClick={() => {
								// @ts-expect-error
								window.__TAURI_INVOKE__('reset_spacedrive');
							}}
						>
							Reset Spacedrive
						</Button>
						{isDev && (
							<p className="mt-2 font-bold">
								You need to manually reset the app after pressing this button in the
								dev environment. This however works seamlessly in production.
							</p>
						)}
					</div>
				)}
			</div>
		</div>
	);
}
