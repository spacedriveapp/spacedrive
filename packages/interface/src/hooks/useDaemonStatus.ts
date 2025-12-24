import { useState, useEffect, useRef } from 'react';
import { usePlatform } from '../platform';
import { useSpacedriveClient } from '../context';
import type { Event } from '@sd/ts-client';

export interface DaemonStatus {
	isConnected: boolean;
	isChecking: boolean;
	isInstalled: boolean;
	isCoreStarted: boolean;
}

export function useDaemonStatus() {
	const platform = usePlatform();
	const client = useSpacedriveClient();
	const [status, setStatus] = useState<DaemonStatus>({
		isConnected: true,
		isChecking: false,
		isInstalled: false,
		isCoreStarted: false,
	});
	const coreStartedRef = useRef(false);

	useEffect(() => {
		if (platform.platform !== 'tauri') {
			return;
		}

		let mounted = true;
		let checkInterval: NodeJS.Timeout | null = null;
		let listenerCleanup: (() => void) | null = null;
		let coreEventUnsubscribe: (() => void) | null = null;

		const checkDaemonStatus = async () => {
			if (!mounted) return;

			try {
				const daemonStatus = await platform.getDaemonStatus?.();
				if (mounted) {
					const isRunning = daemonStatus?.is_running ?? false;
					setStatus(prev => ({
						...prev,
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
			const unlistenConnected = await platform.onDaemonConnected?.(() => {
				console.log('[useDaemonStatus] daemon-connected event received');
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isConnected: true,
						isChecking: false,
					}));

					// Stop polling when connected
					if (checkInterval) {
						clearInterval(checkInterval);
						checkInterval = null;
					}
				}
			});

			const unlistenDisconnected = await platform.onDaemonDisconnected?.(() => {
				console.log('[useDaemonStatus] daemon-disconnected event received');
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isConnected: false,
						isChecking: false,
						isCoreStarted: false,
					}));
					coreStartedRef.current = false;

					// Start polling when disconnected
					if (!checkInterval) {
						checkInterval = setInterval(checkDaemonStatus, 3000);
					}
				}
			});

			const unlistenStarting = await platform.onDaemonStarting?.(() => {
				console.log('[useDaemonStatus] daemon-starting event received');
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isChecking: true,
						isCoreStarted: false,
					}));
					coreStartedRef.current = false;
				}
			});

			// Subscribe to CoreStarted event from the client
			if (client) {
				try {
					const unsubscribe = await client.subscribe((event: Event) => {
						// CoreStarted is a string literal type, not an object
						if (event === 'CoreStarted') {
							console.log('[useDaemonStatus] CoreStarted event received');
							if (mounted && !coreStartedRef.current) {
								coreStartedRef.current = true;
								setStatus(prev => ({
									...prev,
									isCoreStarted: true,
								}));
							}
						}
					});
					coreEventUnsubscribe = unsubscribe;
				} catch (error) {
					console.error('[useDaemonStatus] Failed to subscribe to CoreStarted event:', error);
				}
			}

			return () => {
				unlistenConnected?.();
				unlistenDisconnected?.();
				unlistenStarting?.();
				coreEventUnsubscribe?.();
			};
		};

		// Check if daemon is installed as a service
		const checkInstallation = async () => {
			try {
				const installed = await platform.checkDaemonInstalled?.();
				console.log('[useDaemonStatus] checkInstallation result:', installed);
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isInstalled: installed ?? false,
					}));
				}
			} catch (error) {
				console.error('[useDaemonStatus] Failed to check daemon installation:', error);
			}
		};

		// Initial checks
		checkDaemonStatus();
		checkInstallation();

		// Set up event listeners
		setupListeners()
			.then(cleanup => {
				listenerCleanup = cleanup;
				if (!mounted) {
					listenerCleanup?.();
				}
			})
			.catch(error => {
				console.error('[useDaemonStatus] Failed to set up daemon listeners:', error);
			});

		// Also poll every 5 seconds as a fallback
		const fallbackInterval = setInterval(checkDaemonStatus, 5000);

		return () => {
			mounted = false;
			if (checkInterval) {
				clearInterval(checkInterval);
			}
			clearInterval(fallbackInterval);
			listenerCleanup?.();
		};
	}, [platform, client]);

	const startDaemon = async () => {
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

	const installAndStartDaemon = async (): Promise<boolean> => {
		console.log('[useDaemonStatus] installAndStartDaemon called');
		try {
			console.log('[useDaemonStatus] Calling platform.installDaemonService()');
			await platform.installDaemonService?.();
			console.log('[useDaemonStatus] installDaemonService completed, updating isInstalled state');
			setStatus(prev => ({
				...prev,
				isInstalled: true,
			}));
			return true;
		} catch (error) {
			console.error('[useDaemonStatus] Failed to install daemon service:', error);
			setStatus(prev => ({
				...prev,
				isChecking: false,
			}));
			return false;
		}
	};

	return {
		...status,
		startDaemon,
		installAndStartDaemon,
	};
}
