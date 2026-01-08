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

  confirm(message: string, callback: (result: boolean) => void) {
    callback(window.confirm(message));
  },

  // Web-specific implementations (no native capabilities)
  // File pickers, daemon control, etc. are not available on web
};
