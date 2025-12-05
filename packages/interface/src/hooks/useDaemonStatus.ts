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

		const checkDaemonStatus = async () => {
			if (!mounted) return;

			try {
				const daemonStatus = await platform.getDaemonStatus?.();
				if (mounted) {
					const isRunning = daemonStatus?.is_running ?? false;
					setStatus(prev => ({
						...prev,
						isConnected: isRunning,
						isChecking: false,
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
						isChecking: false,
					}));
				}
			}
		};

		const setupListeners = async () => {
			unlistenConnected = await platform.onDaemonConnected?.(() => {
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isConnected: true,
					}));

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
					}));

					// Start polling when disconnected
					if (!checkInterval) {
						checkInterval = setInterval(checkDaemonStatus, 3000);
					}
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
		};
	}, [platform]);

	const retryConnection = async () => {
		setStatus(prev => ({ ...prev, isChecking: true }));

		try {
			const daemonStatus = await platform.getDaemonStatus?.();
			setStatus({
				isConnected: daemonStatus?.is_running ?? false,
				isChecking: false,
			});

			if (!daemonStatus?.is_running) {
				await platform.startDaemonProcess?.();
				await new Promise(resolve => setTimeout(resolve, 1000));
				const newStatus = await platform.getDaemonStatus?.();
				setStatus({
					isConnected: newStatus?.is_running ?? false,
					isChecking: false,
				});
			}
		} catch (error) {
			setStatus({
				isConnected: false,
				isChecking: false,
			});
		}
	};

	return {
		...status,
		retryConnection,
	};
}
