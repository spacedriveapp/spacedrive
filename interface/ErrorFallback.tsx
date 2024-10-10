import { PropsWithChildren, useEffect, useState } from 'react';
import {
	ErrorBoundary,
	ErrorBoundaryPropsWithComponent,
	FallbackProps
} from 'react-error-boundary';
import { useRouteError } from 'react-router';
import { useDebugState, useTelemetryState } from '@sd/client';
import { Button, Dialogs } from '@sd/ui';

import { showAlertDialog } from './components';
import { useLocale, useOperatingSystem, useTheme } from './hooks';
import { usePlatform } from './util/Platform';

const sentryBrowserLazy = import('@sentry/browser');

const RENDERING_ERROR_LOCAL_STORAGE_KEY = 'was-rendering-error';

export function RouterErrorBoundary() {
	const error = useRouteError();

	const reloadBtn = () => {
		location.reload();
		localStorage.setItem(RENDERING_ERROR_LOCAL_STORAGE_KEY, 'true');
	};

	return (
		<ErrorPage
			message={(error as any).toString()}
			sendReportBtn={() => {
				sentryBrowserLazy.then(({ captureException }) => captureException(error));
				reloadBtn();
			}}
			reloadBtn={reloadBtn}
		/>
	);
}

export default ({ error, resetErrorBoundary }: FallbackProps) => (
	<ErrorPage
		message={`Error: ${error.message}`}
		sendReportBtn={() => {
			sentryBrowserLazy.then(({ captureException }) => captureException(error));
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
	const { telemetryLevelPreference } = useTelemetryState();
	const os = useOperatingSystem();
	const platform = usePlatform();
	const isMacOS = os === 'macOS';
	const [redirecting, _] = useState(() =>
		localStorage.getItem(RENDERING_ERROR_LOCAL_STORAGE_KEY)
	);

	const { t } = useLocale();

	// If the user is on a page and the user presses "Reset" on the error boundary, it may crash in rendering causing the user to get stuck on the error page.
	// If it crashes again, we redirect them instead of infinitely crashing.
	useEffect(() => {
		if (localStorage.getItem(RENDERING_ERROR_LOCAL_STORAGE_KEY) !== null) {
			localStorage.removeItem(RENDERING_ERROR_LOCAL_STORAGE_KEY);
			window.location.pathname = '/';
			console.error(
				'Hit error boundary after reloading. Redirecting to overview screen!',
				redirecting
			);
		}
	});
	if (redirecting) return null; // To stop flash of error boundary after `localStorage` is reset in the first render and the check above starts being `false`

	const resetHandler = () => {
		showAlertDialog({
			title: t('reset'),
			value: t('reset_confirmation'),
			label: t('confirm'),
			cancelBtn: true,
			onSubmit: () => {
				localStorage.clear();
				// @ts-expect-error
				window.__TAURI_INVOKE__('reset_spacedrive');
			}
		});
	};

	if (!submessage && debug.enabled)
		submessage = 'Check the console (CMD/CTRL + OPTION/SHIFT + i) for stack trace.';

	return (
		<div
			data-tauri-drag-region
			role="alert"
			className={
				'flex h-screen w-screen flex-col items-center justify-center border border-app-frame bg-app p-4' +
				(isMacOS ? ' rounded-[10px]' : '')
			}
		>
			<Dialogs />
			<p className="m-3 font-plex text-sm font-bold tracking-wide text-ink-faint">
				{t('app_crashed')}
			</p>
			<h1 className="font-plex text-2xl font-bold tracking-tight text-ink">
				{t('app_crashed_description')}
			</h1>
			<pre className="m-2 max-w-[650px] whitespace-normal text-center text-ink">
				{message}
			</pre>
			{submessage && <pre className="m-2 text-sm text-ink-dull">{submessage}</pre>}
			<div className="flex flex-row space-x-2 text-ink">
				{reloadBtn && (
					<Button variant="accent" className="mt-2" onClick={reloadBtn}>
						{t('reload')}
					</Button>
				)}
				{telemetryLevelPreference !== 'none' && (
					<Button
						variant="gray"
						className="mt-2"
						onClick={() =>
							sendReportBtn
								? sendReportBtn()
								: sentryBrowserLazy.then(({ captureException }) =>
										captureException(message)
									)
						}
					>
						{t('send_report')}
					</Button>
				)}
				{platform.openLogsDir && (
					<Button variant="gray" className="mt-2" onClick={platform.openLogsDir}>
						{t('open_logs')}
					</Button>
				)}

				{(errorsThatRequireACoreReset.includes(message) ||
					message.startsWith('NodeError::FailedToInitializeConfig') ||
					message.startsWith('failed to initialize library manager')) && (
					<div className="flex flex-col items-center pt-12">
						<p className="text-md max-w-[650px] text-center">
							{t('reset_to_continue')}
						</p>
						<p className="mt-3 font-bold"> {t('reset_warning')}</p>
						<Button
							variant="colored"
							onClick={resetHandler}
							className="mt-4 max-w-xs border-transparent bg-red-500"
						>
							{t('reset_and_quit')}
						</Button>
					</div>
				)}
			</div>
		</div>
	);
}

export const BetterErrorBoundary = ({
	children,
	FallbackComponent,
	...props
}: PropsWithChildren<ErrorBoundaryPropsWithComponent>) => {
	useEffect(() => {
		const id = setTimeout(
			() => localStorage.removeItem(RENDERING_ERROR_LOCAL_STORAGE_KEY),
			1000
		);

		return () => clearTimeout(id);
	}, []);

	return (
		<ErrorBoundary FallbackComponent={FallbackComponent} {...props}>
			{children}
		</ErrorBoundary>
	);
};
