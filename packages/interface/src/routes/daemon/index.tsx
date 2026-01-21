import { useEffect, useState } from 'react';
import { Power, Check, Warning, CircleNotch } from '@phosphor-icons/react';
import { usePlatform } from '../../contexts/PlatformContext';

interface DaemonStatus {
	is_running: boolean;
	socket_path: string;
	server_url: string | null;
	started_by_us: boolean;
}

export function DaemonManager() {
	const platform = usePlatform();
	const [status, setStatus] = useState<DaemonStatus | null>(null);
	const [isLoading, setIsLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);
	const [autoStart, setAutoStart] = useState(true);
	const [runInProcess, setRunInProcess] = useState(false);
	const [isStarting, setIsStarting] = useState(false);
	const [isStopping, setIsStopping] = useState(false);

	useEffect(() => {
		checkDaemonStatus();
	}, []);

	async function checkDaemonStatus() {
		if (!platform.getDaemonStatus) return;

		setIsLoading(true);
		setError(null);
		try {
			const daemonStatus = await platform.getDaemonStatus();
			setStatus(daemonStatus);
		} catch (err) {
			setError(err instanceof Error ? err.message : String(err));
			setStatus(null);
		} finally {
			setIsLoading(false);
		}
	}

	async function handleStartDaemon() {
		if (!platform.startDaemonProcess) return;

		setIsStarting(true);
		setError(null);
		try {
			await platform.startDaemonProcess();
			await checkDaemonStatus();
		} catch (err) {
			setError(err instanceof Error ? err.message : String(err));
		} finally {
			setIsStarting(false);
		}
	}

	async function handleStopDaemon() {
		if (!platform.stopDaemonProcess) return;

		setIsStopping(true);
		setError(null);
		try {
			await platform.stopDaemonProcess();
			await checkDaemonStatus();
		} catch (err) {
			setError(err instanceof Error ? err.message : String(err));
		} finally {
			setIsStopping(false);
		}
	}

	async function handleOpenSettings() {
		if (!platform.openMacOSSettings) return;

		try {
			await platform.openMacOSSettings();
		} catch (err) {
			setError(err instanceof Error ? err.message : String(err));
		}
	}

	function getStatusColor() {
		if (isLoading) return 'text-ink-faint';
		return status?.is_running ? 'text-green-500' : 'text-red-500';
	}

	function getStatusIcon() {
		if (isLoading) return CircleNotch;
		return status?.is_running ? Check : Warning;
	}

	const StatusIcon = getStatusIcon();
	const isRunning = status?.is_running || false;

	return (
		<div className="flex flex-col h-full p-6 gap-6 text-ink">
			{/* Header */}
			<div className="flex items-center justify-between">
				<div>
					<h1 className="text-2xl font-semibold text-ink">Daemon Manager</h1>
					<p className="text-sm text-ink-dull mt-1">
						Control the Spacedrive daemon process
					</p>
				</div>
			</div>

			{/* Status Card */}
			<div className="bg-app-box border border-app-line rounded-lg p-4">
				<div className="flex items-center justify-between mb-4">
					<h2 className="text-lg font-medium text-ink">Status</h2>
					<div className="flex items-center gap-2">
						<StatusIcon
							className={`size-5 ${getStatusColor()} ${
								isLoading ? 'animate-spin' : ''
							}`}
							weight="fill"
						/>
						<span className={`text-sm font-medium ${getStatusColor()}`}>
							{isLoading ? 'Checking...' : isRunning ? 'Running' : 'Stopped'}
						</span>
					</div>
				</div>

				{error && (
					<div className="bg-red-500/10 border border-red-500/20 rounded-md p-3 mb-4">
						<p className="text-sm text-red-400">{error}</p>
					</div>
				)}

				<div className="space-y-2 text-sm">
					<div className="flex justify-between">
						<span className="text-ink-dull">Socket Path:</span>
						<span className="text-ink font-mono text-xs">
							{status?.socket_path || 'N/A'}
						</span>
					</div>
					<div className="flex justify-between">
						<span className="text-ink-dull">Server URL:</span>
						<span className="text-ink font-mono text-xs">
							{status?.server_url || 'N/A'}
						</span>
					</div>
					<div className="flex justify-between">
						<span className="text-ink-dull">Started by App:</span>
						<span className="text-ink">
							{status?.started_by_us ? 'Yes' : 'No'}
						</span>
					</div>
				</div>

				<button
					onClick={checkDaemonStatus}
					className="mt-4 w-full px-4 py-2 bg-accent hover:bg-accent-deep text-white rounded-md text-sm font-medium transition-colors"
				>
					Refresh Status
				</button>
			</div>

			{/* Settings Card */}
			<div className="bg-app-box border border-app-line rounded-lg p-4">
				<h2 className="text-lg font-medium text-ink mb-4">Settings</h2>

				<div className="space-y-4">
					{/* Auto-start Toggle */}
					<div className="flex items-center justify-between">
						<div>
							<h3 className="text-sm font-medium text-ink">Auto-start Daemon</h3>
							<p className="text-xs text-ink-dull mt-1">
								Start daemon automatically when app launches
							</p>
						</div>
						<button
							onClick={() => setAutoStart(!autoStart)}
							className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
								autoStart ? 'bg-accent' : 'bg-app-line'
							}`}
						>
							<span
								className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
									autoStart ? 'translate-x-6' : 'translate-x-1'
								}`}
							/>
						</button>
					</div>

					{/* Run in Process Toggle */}
					<div className="flex items-center justify-between">
						<div>
							<h3 className="text-sm font-medium text-ink">Run in Process</h3>
							<p className="text-xs text-ink-dull mt-1">
								Run daemon in the app process (fallback if background permission denied)
							</p>
						</div>
						<button
							onClick={() => setRunInProcess(!runInProcess)}
							className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
								runInProcess ? 'bg-accent' : 'bg-app-line'
							}`}
						>
							<span
								className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
									runInProcess ? 'translate-x-6' : 'translate-x-1'
								}`}
							/>
						</button>
					</div>
				</div>
			</div>

			{/* macOS Background Permission Notice */}
			<div className="bg-sidebar-box border border-sidebar-line rounded-lg p-4">
				<div className="flex items-start gap-3">
					<Warning className="size-5 text-yellow-500 flex-shrink-0 mt-0.5" weight="fill" />
					<div>
						<h3 className="text-sm font-medium text-ink">macOS Background Items</h3>
						<p className="text-xs text-ink-dull mt-1">
							On macOS, running background processes requires permission. If the daemon fails to
							start automatically, check System Settings → General → Login Items & Extensions
							and allow Spacedrive to run in the background.
						</p>
						<button
							onClick={handleOpenSettings}
							className="mt-2 text-xs text-accent hover:text-accent-deep font-medium"
						>
							Open System Settings →
						</button>
					</div>
				</div>
			</div>

			{/* Actions */}
			<div className="flex gap-3">
				<button
					onClick={handleStartDaemon}
					disabled={isRunning || isStarting || isLoading}
					className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-green-500 hover:bg-green-600 disabled:bg-app-line disabled:text-ink-faint text-white rounded-md text-sm font-medium transition-colors"
				>
					{isStarting ? (
						<CircleNotch className="size-4 animate-spin" weight="bold" />
					) : (
						<Power className="size-4" weight="bold" />
					)}
					{isStarting ? 'Starting...' : 'Start Daemon'}
				</button>
				<button
					onClick={handleStopDaemon}
					disabled={!isRunning || isStopping || isLoading || !status?.started_by_us}
					className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-red-500 hover:bg-red-600 disabled:bg-app-line disabled:text-ink-faint text-white rounded-md text-sm font-medium transition-colors"
				>
					{isStopping ? (
						<CircleNotch className="size-4 animate-spin" weight="bold" />
					) : (
						<Power className="size-4" weight="bold" />
					)}
					{isStopping ? 'Stopping...' : 'Stop Daemon'}
				</button>
			</div>
		</div>
	);
}