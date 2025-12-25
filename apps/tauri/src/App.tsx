import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
	Explorer,
	FloatingControls,
	LocationCacheDemo,
	PopoutInspector,
	QuickPreview,
	JobsScreen,
	Settings,
	PlatformProvider,
	SpacedriveProvider,
	ServerProvider,
} from "@sd/interface";
import {
	SpacedriveClient,
	TauriTransport,
	useSyncPreferencesStore,
} from "@sd/ts-client";
import type { Event as CoreEvent } from "@sd/ts-client";
import { sounds } from "@sd/assets/sounds";
import { useEffect, useState } from "react";
import { DragOverlay } from "./routes/DragOverlay";
import { ContextMenuWindow } from "./routes/ContextMenuWindow";
import { DragDemo } from "./components/DragDemo";
import { SpacedropWindow } from "./routes/Spacedrop";
import { platform } from "./platform";
import { initializeContextMenuHandler } from "./contextMenu";
import { initializeKeybindGlobal } from "./keybinds";

function App() {
	const [client, setClient] = useState<SpacedriveClient | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [route, setRoute] = useState<string>("/");

	useEffect(() => {
		// React Scan disabled - too heavy for development
		// Uncomment if you need to debug render performance:
		if (import.meta.env.DEV) {
			// setTimeout(() => {
			// 	import("react-scan").then(({ scan }) => {
			// 		scan({ enabled: true, log: false });
			// 	});
			// }, 2000);
		}

		// Initialize Tauri native context menu handler
		initializeContextMenuHandler();

		// Initialize Tauri keybind handler
		initializeKeybindGlobal();

		// Prevent default context menu globally (except in context menu windows)
		const currentWindow = getCurrentWebviewWindow();
		const label = currentWindow.label;

		// Prevent default browser context menu globally (except in context menu windows)
		if (!label.startsWith("context-menu")) {
			const preventContextMenu = (e: Event) => {
				// Default behavior: prevent browser context menu
				// React's onContextMenu handlers can override this with their own preventDefault
				e.preventDefault();
			};
			document.addEventListener("contextmenu", preventContextMenu, {
				capture: false,
			});
		}

		// Set route based on window label
		if (label === "floating-controls") {
			setRoute("/floating-controls");
		} else if (label.startsWith("drag-overlay")) {
			setRoute("/drag-overlay");
		} else if (label.startsWith("context-menu")) {
			setRoute("/contextmenu");
		} else if (label.startsWith("drag-demo")) {
			setRoute("/drag-demo");
		} else if (label.startsWith("spacedrop")) {
			setRoute("/spacedrop");
		} else if (label.startsWith("settings")) {
			setRoute("/settings");
		} else if (label.startsWith("inspector")) {
			setRoute("/inspector");
		} else if (label.startsWith("quick-preview")) {
			setRoute("/quick-preview");
		} else if (label.startsWith("cache-demo")) {
			setRoute("/cache-demo");
		} else if (label.startsWith("job-manager")) {
			setRoute("/job-manager");
		}

		// Tell Tauri window is ready to be shown
		invoke("app_ready").catch(console.error);

		// Play startup sound
		// sounds.startup();

		let unsubscribePromise: Promise<() => void> | null = null;

		// Create Tauri-based client
		try {
			const transport = new TauriTransport(invoke, listen);
			const spacedrive = new SpacedriveClient(transport);
			setClient(spacedrive);

			// Query current library ID from platform state (for popout windows)
			if (platform.getCurrentLibraryId) {
				platform
					.getCurrentLibraryId()
					.then((libraryId) => {
						if (libraryId) {
							spacedrive.setCurrentLibrary(libraryId, false); // Don't emit - already in sync
						}
					})
					.catch(() => {
						// Library not selected yet - this is fine for initial load
					});
			}

			// Listen for library-changed events via platform (emitted when library switches)
			if (platform.onLibraryIdChanged) {
				platform.onLibraryIdChanged((newLibraryId) => {
					spacedrive.setCurrentLibrary(newLibraryId, true); // DO emit - hooks need to know!
				});
			}

			// Subscribe to core events for auto-switching on synced library creation
			unsubscribePromise = spacedrive.subscribe((event: CoreEvent) => {
				// Check if this is a LibraryCreated event from sync
				if (
					typeof event === "object" &&
					"LibraryCreated" in event &&
					(event.LibraryCreated as any).source === "Sync"
				) {
					const { id, name } = event.LibraryCreated;

					// Check user preference for auto-switching
					const autoSwitchEnabled =
						useSyncPreferencesStore.getState().autoSwitchOnSync;

					if (autoSwitchEnabled) {
						console.log(
							`[Auto-Switch] Received synced library "${name}", switching...`,
						);

						// Switch to the new library via platform (syncs across all windows)
						if (platform.setCurrentLibraryId) {
							platform.setCurrentLibraryId(id).catch((err) => {
								console.error(
									"[Auto-Switch] Failed to switch library:",
									err,
								);
							});
						} else {
							// Fallback: just update the client
							spacedrive.setCurrentLibrary(id);
						}
					} else {
						console.log(
							`[Auto-Switch] Received synced library "${name}", but auto-switch is disabled`,
						);
					}
				}
			});

			// No global subscription needed - each useNormalizedCache creates its own filtered subscription
		} catch (err) {
			console.error("Failed to create client:", err);
			setError(err instanceof Error ? err.message : String(err));
		}

		return () => {
			if (unsubscribePromise) {
				unsubscribePromise.then((unsubscribe) => unsubscribe());
			}

			// Clean up all backend TCP connections to prevent connection leaks
			// This is especially important during development hot reloads
			invoke("cleanup_all_connections").catch((err) => {
				console.warn("Failed to cleanup connections:", err);
			});
		};
	}, []);

	// Routes that don't need the client
	if (route === "/floating-controls") {
		return <FloatingControls />;
	}

	if (route === "/drag-overlay") {
		return <DragOverlay />;
	}

	if (route === "/contextmenu") {
		return <ContextMenuWindow />;
	}

	if (route === "/drag-demo") {
		return <DragDemo />;
	}

	if (route === "/spacedrop") {
		return <SpacedropWindow />;
	}

	if (error) {
		console.log("Rendering error state");
		return (
			<div className="flex h-screen items-center justify-center bg-gray-950 text-white">
				<div className="text-center">
					<h1 className="text-2xl font-bold mb-4">Error</h1>
					<p className="text-red-400">{error}</p>
				</div>
			</div>
		);
	}

	if (!client) {
		console.log("Rendering loading state");
		return (
			<div className="flex h-screen items-center justify-center bg-gray-950 text-white">
				<div className="text-center">
					<div className="animate-pulse text-xl">
						Initializing client...
					</div>
					<p className="text-gray-400 text-sm mt-2">
						Check console for logs
					</p>
				</div>
			</div>
		);
	}

	console.log("Rendering Interface with client");

	// Route to different UIs based on window type
	if (route === "/settings") {
		return (
			<PlatformProvider platform={platform}>
				<SpacedriveProvider client={client}>
					<Settings />
				</SpacedriveProvider>
			</PlatformProvider>
		);
	}

	if (route === "/inspector") {
		return (
			<PlatformProvider platform={platform}>
				<SpacedriveProvider client={client}>
					<ServerProvider>
						<div className="h-screen bg-app overflow-hidden">
							<PopoutInspector />
						</div>
					</ServerProvider>
				</SpacedriveProvider>
			</PlatformProvider>
		);
	}

	if (route === "/cache-demo") {
		return <LocationCacheDemo />;
	}

	if (route === "/quick-preview") {
		return (
			<PlatformProvider platform={platform}>
				<SpacedriveProvider client={client}>
					<ServerProvider>
						<div className="h-screen bg-app overflow-hidden">
							<QuickPreview />
						</div>
					</ServerProvider>
				</SpacedriveProvider>
			</PlatformProvider>
		);
	}

	if (route === "/job-manager") {
		return (
			<PlatformProvider platform={platform}>
				<SpacedriveProvider client={client}>
					<ServerProvider>
						<div className="h-screen bg-app overflow-hidden rounded-[10px] border border-transparent frame">
							<JobsScreen />
						</div>
					</ServerProvider>
				</SpacedriveProvider>
			</PlatformProvider>
		);
	}

	return (
		<PlatformProvider platform={platform}>
			<Explorer client={client} />
		</PlatformProvider>
	);
}

export default App;
