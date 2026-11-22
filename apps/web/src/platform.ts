import type { Platform } from "@sd/interface/platform";

/**
 * Web platform implementation for Spacedrive server
 *
 * This provides a minimal platform abstraction for the web client.
 * Unlike Tauri, web platform cannot access native file system or daemon state directly.
 */
export const platform: Platform = {
	platform: "web",

	openLink(url: string) {
		window.open(url, "_blank", "noopener,noreferrer");
	},

	// sd-server serves the UI, RPC, and sidecars from a single origin, so the
	// sidecar base URL is wherever this page was loaded from. Without this the
	// ServerContext never gets a serverUrl and thumbnails silently never load.
	async getDaemonStatus() {
		return {
			is_running: true,
			socket_path: "",
			server_url: window.location.origin,
			started_by_us: false,
		};
	},

	// The mounts share listens on the daemon's loopback; sd-server proxies it
	// under the same path on this origin, behind its own auth.
	shareUrl(url: string) {
		const { pathname, search } = new URL(url);
		return pathname + search;
	},

	confirm(message: string, callback: (result: boolean) => void) {
		callback(window.confirm(message));
	},

	// Web-specific implementations (no native capabilities)
	// File pickers, daemon control, etc. are not available on web
};
