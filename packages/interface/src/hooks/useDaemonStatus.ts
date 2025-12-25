import { useState, useEffect, useRef } from 'react';
import { usePlatform } from '../platform';

export interface DaemonStatus {
	isConnected: boolean;
	isChecking: boolean;
	isInstalled: boolean;
	/** True during initial app startup while waiting for daemon. Once connected, stays false. */
	isStarting: boolean;
}

export function useDaemonStatus() {
	const platform = usePlatform();
	// For Tauri, start in "starting" state. For web, assume connected.
	const isTauri = platform.platform === 'tauri';
	const [status, setStatus] = useState<DaemonStatus>({
		isConnected: !isTauri, // Web is always "connected", Tauri starts disconnected
		isChecking: false,
		isInstalled: false,
		isStarting: isTauri, // Only Tauri starts in "starting" state
	});
	
	// Track if we've ever been connected - once connected, isStarting stays false
	const hasEverConnected = useRef(!isTauri);

	useEffect(() => {
		if (platform.platform !== 'tauri') {
			return;
		}

		let mounted = true;
		let listenerCleanup: (() => void) | null = null;

		const checkDaemonStatus = async () => {
			if (!mounted) return;

			try {
				const daemonStatus = await platform.getDaemonStatus?.();
				if (mounted) {
					const isRunning = daemonStatus?.is_running ?? false;
					
					if (isRunning) {
						hasEverConnected.current = true;
					}
					
				setStatus(prev => ({
					...prev,
					isConnected: isRunning,
					// Only clear isChecking if we're connected (daemon started successfully)
					isChecking: isRunning ? false : prev.isChecking,
					// Clear isStarting once we're connected
					isStarting: isRunning ? false : prev.isStarting,
				}));
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
					hasEverConnected.current = true;
				setStatus(prev => ({
					...prev,
					isConnected: true,
					isChecking: false,
					isStarting: false, // No longer starting once connected
				}));
				}
			});

			const unlistenDisconnected = await platform.onDaemonDisconnected?.(() => {
				console.log('[useDaemonStatus] daemon-disconnected event received');
				if (mounted) {
				setStatus(prev => ({
					...prev,
					isConnected: false,
					isChecking: false,
					// If we were ever connected before, this is a disconnection, not startup
					// Keep isStarting as is - only clear it on connect
					isStarting: hasEverConnected.current ? false : prev.isStarting,
				}));

				// Don't create additional polling - fallback interval already running
				}
			});

			const unlistenStarting = await platform.onDaemonStarting?.(() => {
				console.log('[useDaemonStatus] daemon-starting event received');
				if (mounted) {
					setStatus(prev => ({
						...prev,
						isChecking: true,
						// If daemon is starting (e.g., user clicked restart), show startup state
						// But only if we haven't connected yet in this session
						isStarting: !hasEverConnected.current,
					}));
				}
			});

			return () => {
				unlistenConnected?.();
				unlistenDisconnected?.();
				unlistenStarting?.();
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

		// Fallback polling only when disconnected (event listeners should handle normal case)
		// Start with 3 second interval on startup
		const fallbackInterval = setInterval(checkDaemonStatus, 3000);

		return () => {
			mounted = false;
			clearInterval(fallbackInterval);
			listenerCleanup?.();
		};
	}, [platform]);

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
