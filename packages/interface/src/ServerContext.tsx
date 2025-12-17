import {
	createContext,
	useContext,
	useState,
	useEffect,
	type ReactNode,
} from "react";
import { usePlatform } from "./platform";
import { useClient } from "./context";

/**
 * Server context provides reactive access to the daemon server URL and current library ID.
 *
 * This replaces the unreliable window.__SPACEDRIVE_SERVER_URL__ and __SPACEDRIVE_LIBRARY_ID__
 * globals that were injected via Tauri's window.eval(). The old approach had race conditions
 * where components would render before the injection completed, resulting in null values.
 */

export interface ServerContextValue {
	/** Base URL of the daemon HTTP server (e.g., "http://localhost:9420") */
	serverUrl: string | null;
	/** Currently active library ID */
	libraryId: string | null;
	/** Whether both serverUrl and libraryId are available */
	isReady: boolean;
	/**
	 * Build a sidecar URL for fetching thumbnails, thumbstrips, transcripts, etc.
	 * Returns null if serverUrl or libraryId is not available.
	 */
	buildSidecarUrl: (
		contentUuid: string,
		kind: string,
		variant: string,
		format: string,
	) => string | null;
}

const ServerContext = createContext<ServerContextValue | null>(null);

export interface ServerProviderProps {
	children: ReactNode;
}

/**
 * Provider that manages server URL and library ID state.
 *
 * Gets initial values from the platform and listens for changes.
 * Must be rendered inside PlatformProvider and SpacedriveProvider.
 */
export function ServerProvider({ children }: ServerProviderProps) {
	const platform = usePlatform();
	const client = useClient();

	const [serverUrl, setServerUrl] = useState<string | null>(null);
	const [libraryId, setLibraryId] = useState<string | null>(() => {
		// Initialize from client if already set
		return client.getCurrentLibraryId();
	});

	// Get initial server URL from platform
	useEffect(() => {
		if (platform.getDaemonStatus) {
			platform
				.getDaemonStatus()
				.then((status) => {
					if (status.server_url) {
						setServerUrl(status.server_url);
					}
				})
				.catch((err) => {
					console.warn(
						"[ServerContext] Failed to get daemon status:",
						err,
					);
				});
		}
	}, [platform]);

	// Get initial library ID from platform (may differ from client state)
	useEffect(() => {
		if (platform.getCurrentLibraryId) {
			platform
				.getCurrentLibraryId()
				.then((id) => {
					if (id) {
						setLibraryId(id);
					}
				})
				.catch(() => {
					// Library not selected yet - this is fine
				});
		}
	}, [platform]);

	// Listen for library ID changes via platform events
	useEffect(() => {
		if (platform.onLibraryIdChanged) {
			const unlistenPromise = platform.onLibraryIdChanged(
				(newLibraryId) => {
					setLibraryId(newLibraryId);
				},
			);

			return () => {
				unlistenPromise.then((unlisten) => unlisten());
			};
		}
	}, [platform]);

	// Listen for library changes via client events
	useEffect(() => {
		const handleLibraryChange = (newLibraryId: string) => {
			setLibraryId(newLibraryId);
		};

		client.on("library-changed", handleLibraryChange);
		return () => {
			client.off("library-changed", handleLibraryChange);
		};
	}, [client]);

	// Listen for daemon connection events to update server URL
	useEffect(() => {
		if (platform.onDaemonConnected && platform.getDaemonStatus) {
			const unlistenPromise = platform.onDaemonConnected(() => {
				// Re-fetch daemon status when connection established
				platform.getDaemonStatus!().then((status) => {
					if (status.server_url) {
						setServerUrl(status.server_url);
					}
				});
			});

			return () => {
				unlistenPromise.then((unlisten) => unlisten());
			};
		}
	}, [platform]);

	const buildSidecarUrl = (
		contentUuid: string,
		kind: string,
		variant: string,
		format: string,
	): string | null => {
		if (!serverUrl || !libraryId) {
			return null;
		}
		return `${serverUrl}/sidecar/${libraryId}/${contentUuid}/${kind}/${variant}.${format}`;
	};

	const value: ServerContextValue = {
		serverUrl,
		libraryId,
		isReady: serverUrl !== null && libraryId !== null,
		buildSidecarUrl,
	};

	return (
		<ServerContext.Provider value={value}>
			{children}
		</ServerContext.Provider>
	);
}

/**
 * Hook to access server URL and library ID.
 *
 * Must be used within a ServerProvider.
 *
 * @example
 * ```tsx
 * function Thumbnail({ file }) {
 *   const { buildSidecarUrl, isReady } = useServer();
 *
 *   if (!isReady) return <Skeleton />;
 *
 *   const thumbUrl = buildSidecarUrl(
 *     file.content_identity.uuid,
 *     "thumb",
 *     "grid@1x",
 *     "webp"
 *   );
 *
 *   return <img src={thumbUrl} />;
 * }
 * ```
 */
export function useServer(): ServerContextValue {
	const context = useContext(ServerContext);

	if (!context) {
		throw new Error(
			"useServer must be used within a ServerProvider. " +
				"Make sure ServerProvider is mounted above this component.",
		);
	}

	return context;
}
