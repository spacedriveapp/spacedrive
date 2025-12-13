import { open, save } from "@tauri-apps/plugin-dialog";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { convertFileSrc as tauriConvertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Platform } from "@sd/interface/platform";
import { beginDrag, onDragBegan, onDragMoved, onDragEntered, onDragLeft, onDragEnded } from "./lib/drag";
import { registerTauriKeybind, unregisterTauriKeybind } from "./keybinds";

let _isDragging = false;

/**
 * Tauri platform implementation
 */
export const platform: Platform = {
	platform: "tauri",

	async openDirectoryPickerDialog(opts) {
		const result = await open({
			directory: true,
			multiple: opts?.multiple ?? false,
			title: opts?.title ?? "Choose a folder",
		});

		return result;
	},

	async openFilePickerDialog(opts) {
		const result = await open({
			directory: false,
			multiple: opts?.multiple ?? false,
			title: opts?.title ?? "Choose a file",
		});

		return result;
	},

	async saveFilePickerDialog(opts) {
		const result = await save({
			title: opts?.title ?? "Save file",
			defaultPath: opts?.defaultPath,
		});

		return result;
	},

	openLink(url: string) {
		shellOpen(url);
	},

	confirm(message: string, callback: (result: boolean) => void) {
		// Use browser confirm for now - could be replaced with custom dialog
		callback(window.confirm(message));
	},

	convertFileSrc(filePath: string) {
		return tauriConvertFileSrc(filePath);
	},

	async revealFile(filePath: string) {
		await invoke("reveal_file", { path: filePath });
	},

	async getSidecarPath(
		libraryId: string,
		contentUuid: string,
		kind: string,
		variant: string,
		format: string
	) {
		return await invoke<string>("get_sidecar_path", {
			libraryId,
			contentUuid,
			kind,
			variant,
			format,
		});
	},

	async updateMenuItems(items) {
		await invoke("update_menu_items", { items });
	},

	async getCurrentLibraryId() {
		try {
			return await invoke<string>("get_current_library_id");
		} catch {
			return null;
		}
	},

	async setCurrentLibraryId(libraryId: string) {
		await invoke("set_current_library_id", { libraryId });
	},

	async onLibraryIdChanged(callback: (libraryId: string) => void) {
		const unlisten = await listen<string>("library-changed", (event) => {
			callback(event.payload);
		});
		return unlisten;
	},

	async showWindow(window: any) {
		await invoke("show_window", { window });
	},

	async closeWindow(label: string) {
		await invoke("close_window", { label });
	},

	async onWindowEvent(event: string, callback: () => void) {
		const unlisten = await listen(event, () => {
			callback();
		});
		return unlisten;
	},

	getCurrentWindowLabel() {
		const window = getCurrentWebviewWindow();
		return window.label;
	},

	async closeCurrentWindow() {
		const window = getCurrentWebviewWindow();
		await window.close();
	},

	async getSelectedFileIds() {
		return await invoke<string[]>("get_selected_file_ids");
	},

	async setSelectedFileIds(fileIds: string[]) {
		await invoke("set_selected_file_ids", { fileIds });
	},

	async onSelectedFilesChanged(callback: (fileIds: string[]) => void) {
		const unlisten = await listen<string[]>("selected-files-changed", (event) => {
			callback(event.payload);
		});
		return unlisten;
	},

	async getDaemonStatus() {
		return await invoke<{
			is_running: boolean;
			socket_path: string;
			server_url: string | null;
			started_by_us: boolean;
		}>("get_daemon_status");
	},

	async startDaemonProcess() {
		await invoke("start_daemon_process");
	},

	async stopDaemonProcess() {
		await invoke("stop_daemon_process");
	},

	async onDaemonConnected(callback: () => void) {
		const unlisten = await listen("daemon-connected", () => {
			callback();
		});
		return unlisten;
	},

	async onDaemonDisconnected(callback: () => void) {
		const unlisten = await listen("daemon-disconnected", () => {
			callback();
		});
		return unlisten;
	},

	async onDaemonStarting(callback: () => void) {
		const unlisten = await listen("daemon-starting", () => {
			callback();
		});
		return unlisten;
	},

	async checkDaemonInstalled() {
		return await invoke<boolean>("check_daemon_installed");
	},

	async installDaemonService() {
		await invoke("install_daemon_service");
	},

	async uninstallDaemonService() {
		await invoke("uninstall_daemon_service");
	},

	async openMacOSSettings() {
		await invoke("open_macos_settings");
	},

	async startDrag(config) {
		const currentWindow = getCurrentWebviewWindow();
		const sessionId = await beginDrag(
			{
				items: config.items.map(item => ({
					id: item.id,
					kind: item.kind,
				})),
				overlayUrl: "/drag-overlay",
				overlaySize: [200, 150],
				allowedOperations: config.allowedOperations,
			},
			currentWindow.label
		);
		_isDragging = true;
		return sessionId;
	},

	async onDragEvent(event, callback) {
		const handlers: Record<string, typeof onDragBegan> = {
			began: onDragBegan,
			moved: onDragMoved,
			entered: onDragEntered,
			left: onDragLeft,
			ended: onDragEnded,
		};
		const handler = handlers[event];
		if (!handler) {
			throw new Error(`Unknown drag event: ${event}`);
		}
		const unlisten = await handler((payload: any) => {
			if (event === "ended") {
				_isDragging = false;
			}
			callback(payload);
		});
		return unlisten;
	},

	isDragging() {
		return _isDragging;
	},

	// Keybind registration (uses web listener for now, can be extended to native shortcuts)
	registerKeybind: registerTauriKeybind,
	unregisterKeybind: unregisterTauriKeybind,
};
