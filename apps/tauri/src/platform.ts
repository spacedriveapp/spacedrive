import { open, save } from "@tauri-apps/plugin-dialog";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { convertFileSrc as tauriConvertFileSrc } from "@tauri-apps/api/core";
import type { Platform } from "@sd/interface/platform";

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
};
