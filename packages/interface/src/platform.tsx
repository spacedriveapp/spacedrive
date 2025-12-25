import { createContext, useContext, PropsWithChildren } from "react";

/**
 * Platform abstraction layer
 *
 * This allows the interface to remain platform-agnostic while providing
 * platform-specific functionality like file pickers, native commands, etc.
 */
export type Platform = {
	/** Platform discriminator */
	platform: "web" | "tauri";

	/** Open native directory picker dialog (Tauri only) */
	openDirectoryPickerDialog?(opts?: {
		title?: string;
		multiple?: boolean;
	}): Promise<string | string[] | null>;

	/** Open native file picker dialog (Tauri only) */
	openFilePickerDialog?(opts?: {
		title?: string;
		multiple?: boolean;
	}): Promise<string | string[] | null>;

	/** Save file picker dialog (Tauri only) */
	saveFilePickerDialog?(opts?: {
		title?: string;
		defaultPath?: string;
	}): Promise<string | null>;

	/** Open a URL in the default browser */
	openLink(url: string): void;

	/** Show native confirmation dialog */
	confirm(message: string, callback: (result: boolean) => void): void;

	/** Convert a file path to a URL that can be loaded in the webview */
	convertFileSrc?(filePath: string): string;

	/** Reveal a file in the native file manager (Finder on macOS, Explorer on Windows, etc.) */
	revealFile?(filePath: string): Promise<void>;

	/** Get applications that can open the given file paths (intersection for multiple files) */
	getAppsForPaths?(paths: string[]): Promise<OpenWithApp[]>;

	/** Open file with system default application */
	openPathDefault?(path: string): Promise<OpenResult>;

	/** Open file with specific application */
	openPathWithApp?(path: string, appId: string): Promise<OpenResult>;

	/** Open multiple files with specific application */
	openPathsWithApp?(paths: string[], appId: string): Promise<OpenResult[]>;

	/** Get the physical path to a sidecar file */
	getSidecarPath?(
		libraryId: string,
		contentUuid: string,
		kind: string,
		variant: string,
		format: string
	): Promise<string>;

	/** Update native menu item states (Tauri only) */
	updateMenuItems?(items: MenuItemState[]): Promise<void>;

	/** Get the current library ID from platform state (Tauri global state) */
	getCurrentLibraryId?(): Promise<string | null>;

	/** Set the current library ID in platform state and sync to all windows (Tauri only) */
	setCurrentLibraryId?(libraryId: string): Promise<void>;

	/** Listen for library ID changes across all windows (Tauri only) */
	onLibraryIdChanged?(callback: (libraryId: string) => void): Promise<() => void>;

	/** Show a specific window type (Tauri only) */
	showWindow?(window: any): Promise<void>;

	/** Close a window by label (Tauri only) */
	closeWindow?(label: string): Promise<void>;

	/** Listen for window events (Tauri only) */
	onWindowEvent?(event: string, callback: () => void): Promise<() => void>;

	/** Get current window label (Tauri only) */
	getCurrentWindowLabel?(): string;

	/** Close current window (Tauri only) */
	closeCurrentWindow?(): Promise<void>;

	/** Get currently selected file IDs from platform state (Tauri only) */
	getSelectedFileIds?(): Promise<string[]>;

	/** Set selected file IDs in platform state and sync to all windows (Tauri only) */
	setSelectedFileIds?(fileIds: string[]): Promise<void>;

	/** Listen for selected file changes across all windows (Tauri only) */
	onSelectedFilesChanged?(callback: (fileIds: string[]) => void): Promise<() => void>;

	/** Get app version (Tauri only) */
	getAppVersion?(): Promise<string>;

	/** Get daemon status (Tauri only) */
	getDaemonStatus?(): Promise<{
		is_running: boolean;
		socket_path: string;
		server_url: string | null;
		started_by_us: boolean;
	}>;

	/** Start daemon process (Tauri only) */
	startDaemonProcess?(): Promise<void>;

	/** Stop daemon process (Tauri only) */
	stopDaemonProcess?(): Promise<void>;

	/** Listen for daemon connection events (Tauri only) */
	onDaemonConnected?(callback: () => void): Promise<() => void>;

	/** Listen for daemon disconnection events (Tauri only) */
	onDaemonDisconnected?(callback: () => void): Promise<() => void>;

	/** Listen for daemon starting events (Tauri only) */
	onDaemonStarting?(callback: () => void): Promise<() => void>;

	/** Check if daemon is installed as a service (Tauri only) */
	checkDaemonInstalled?(): Promise<boolean>;

	/** Install daemon as a service (Tauri only) */
	installDaemonService?(): Promise<void>;

	/** Uninstall daemon service (Tauri only) */
	uninstallDaemonService?(): Promise<void>;

	/** Open macOS system settings (Tauri/macOS only) */
	openMacOSSettings?(): Promise<void>;

	// Drag and Drop API (Tauri only)

	/** Start a native drag operation */
	startDrag?(config: {
		items: Array<{
			id: string;
			kind: { type: "file"; path: string } | { type: "text"; content: string };
		}>;
		allowedOperations: Array<"copy" | "move" | "link">;
	}): Promise<string>;

	/** Listen for drag events */
	onDragEvent?(
		event: "began" | "moved" | "entered" | "left" | "ended",
		callback: (payload: any) => void
	): Promise<() => void>;

	/** Check if a drag operation is in progress */
	isDragging?(): boolean;

	// Keybind API

	/** Register a keybind handler (Tauri only) */
	registerKeybind?(
		id: string,
		accelerator: string,
		handler: () => void | Promise<void>
	): Promise<void>;

	/** Unregister a keybind handler (Tauri only) */
	unregisterKeybind?(id: string): Promise<void>;
};

/** Application that can open a file */
export interface OpenWithApp {
	/** Platform-specific identifier (bundle ID on macOS, app name on Windows, desktop entry on Linux) */
	id: string;
	/** Human-readable display name */
	name: string;
	/** Optional base64-encoded icon */
	icon?: string;
}

/** Result of opening a file */
export type OpenResult =
	| { status: "success" }
	| { status: "file_not_found"; path: string }
	| { status: "app_not_found"; app_id: string }
	| { status: "permission_denied"; path: string }
	| { status: "platform_error"; message: string };

/** Menu item state for native menus */
export interface MenuItemState {
	/** Unique identifier for the menu item */
	id: string;
	/** Whether the menu item is enabled */
	enabled: boolean;
}

const PlatformContext = createContext<Platform | undefined>(undefined);

export function usePlatform(): Platform {
	const ctx = useContext(PlatformContext);
	if (!ctx) {
		throw new Error(
			"usePlatform must be used within a PlatformProvider. Make sure PlatformProvider is mounted above this component."
		);
	}
	return ctx;
}

export function PlatformProvider({
	platform,
	children,
}: PropsWithChildren<{ platform: Platform }>) {
	return <PlatformContext.Provider value={platform}>{children}</PlatformContext.Provider>;
}
