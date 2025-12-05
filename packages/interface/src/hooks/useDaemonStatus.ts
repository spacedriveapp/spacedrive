import { useState, useEffect } from 'react';
import { usePlatform } from '../platform';

export interface DaemonStatus {
	isConnected: boolean;
	isChecking: boolean;
}

export function useDaemonStatus() {
	const platform = usePlatform();
	const [status, setStatus] = useState<DaemonStatus>({
		isConnected: true,
		isChecking: false,
	});

	useEffect(() => {
		if (platform.platform !== 'tauri') {
			return;
		}

		let mounted = true;
		let checkInterval: NodeJS.Timeout | null = null;
		let unlistenConnected: (() => void) | undefined;
		let unlistenDisconnected: (() => void) | undefined;
		let unlistenStarting: (() => void) | undefined;

		const checkDaemonStatus = async () => {
			if (!mounted) return;

			try {
				const daemonStatus = await platform.getDaemonStatus?.();
				if (mounted) {
					const isRunning = daemonStatus?.is_running ?? false;
					setStatus(prev => ({
						isConnected: isRunning,
						// Only clear isChecking if we're connected (daemon started successfully)
						isChecking: isRunning ? false : prev.isChecking,
					}));

					// Clear polling if daemon is back online
					if (isRunning && checkInterval) {
						clearInterval(checkInterval);
						checkInterval = null;
					}
				}
			} catch (error) {
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isConnected: false,
						// Don't clear isChecking on error - might still be starting
					}));
				}
			}
		};

		const setupListeners = async () => {
			unlistenConnected = await platform.onDaemonConnected?.(() => {
				if (mounted) {
					setStatus({
						isConnected: true,
						isChecking: false,
					});

					// Stop polling when connected
					if (checkInterval) {
						clearInterval(checkInterval);
						checkInterval = null;
					}
				}
			});

			unlistenDisconnected = await platform.onDaemonDisconnected?.(() => {
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isConnected: false,
						isChecking: false,
					}));

					// Start polling when disconnected
					if (!checkInterval) {
						checkInterval = setInterval(checkDaemonStatus, 3000);
					}
				}
			});

			unlistenStarting = await platform.onDaemonStarting?.(() => {
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isChecking: true,
					}));
				}
			});
		};

		// Initial check
		checkDaemonStatus();

		// Set up event listeners
		setupListeners();

		// Also poll every 5 seconds as a fallback
		const fallbackInterval = setInterval(checkDaemonStatus, 5000);

		return () => {
			mounted = false;
			if (checkInterval) {
				clearInterval(checkInterval);
			}
			clearInterval(fallbackInterval);
			unlistenConnected?.();
			unlistenDisconnected?.();
			unlistenStarting?.();
		};
	}, [platform]);

	const retryConnection = async () => {
		try {
			await platform.startDaemonProcess?.();
		} catch (error) {
			console.error('Failed to start daemon:', error);
			setStatus(prev => ({
				...prev,
				isChecking: false,
			}));
		}
	};

	return {
		...status,
		retryConnection,
	};
}
