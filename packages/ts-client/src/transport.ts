/**
 * Platform-agnostic transport layer for Spacedrive client
 * Uses Bun APIs for Unix sockets and Tauri invoke for browser
 */

import { DEFAULT_EVENT_SUBSCRIPTION } from "./event-filter";

export interface Transport {
	sendRequest(request: any): Promise<any>;
	subscribe(callback: (event: any) => void): Promise<() => void>;
}

/**
 * Tauri transport using IPC invoke
 * This works in the browser (Tauri webview)
 */
export class TauriTransport implements Transport {
	private invoke: (cmd: string, args?: any) => Promise<any>;
	private listen: (event: string, handler: (event: any) => void) => Promise<() => void>;

	constructor(
		invoke: (cmd: string, args?: any) => Promise<any>,
		listen: (event: string, handler: (event: any) => void) => Promise<() => void>
	) {
		this.invoke = invoke;
		this.listen = listen;
	}

	async sendRequest(request: any): Promise<any> {
		const response = await this.invoke("daemon_request", { request });
		return response;
	}

	async subscribe(callback: (event: any) => void): Promise<() => void> {
		// Start the event subscription on the backend
		await this.invoke("subscribe_to_events");

		// Listen to forwarded events from Tauri
		const unlisten = await this.listen("core-event", (tauriEvent: any) => {
			callback(tauriEvent.payload);
		});

		return unlisten;
	}
}

/**
 * Unix socket transport for Bun/Node environments
 * Note: This won't work in browser, use TauriTransport instead
 */
export class UnixSocketTransport implements Transport {
	constructor(private socketPath: string) {}

	async sendRequest(request: any): Promise<any> {
		// This uses Bun.connect which only works in Bun runtime
		// @ts-ignore - Bun global
		const socket = await Bun.connect({
			unix: this.socketPath,
		});

		const requestLine = JSON.stringify(request) + "\n";
		await socket.write(requestLine);

		// Read response
		const reader = socket.reader;
		let buffer = "";

		for await (const chunk of reader) {
			buffer += new TextDecoder().decode(chunk);

			const newlineIndex = buffer.indexOf("\n");
			if (newlineIndex !== -1) {
				const line = buffer.slice(0, newlineIndex).trim();
				socket.end();
				return JSON.parse(line);
			}
		}

		throw new Error("Connection closed without response");
	}

	async subscribe(callback: (event: any) => void): Promise<() => void> {
		// @ts-ignore - Bun global
		const socket = await Bun.connect({
			unix: this.socketPath,
		});

		// Subscribe to relevant events (excludes spammy LogMessage/JobProgress)
		const subscribeRequest = {
			Subscribe: {
				event_types: DEFAULT_EVENT_SUBSCRIPTION,
				filter: null,
			},
		};

		await socket.write(JSON.stringify(subscribeRequest) + "\n");

		// Listen for events
		const reader = socket.reader;
		let buffer = "";

		(async () => {
			for await (const chunk of reader) {
				buffer += new TextDecoder().decode(chunk);

				const lines = buffer.split("\n");
				buffer = lines.pop() || "";

				for (const line of lines) {
					if (!line.trim()) continue;

					try {
						const response = JSON.parse(line);
						if ("Event" in response) {
							callback(response.Event);
						}
					} catch (error) {
						console.error("Failed to parse event:", error);
					}
				}
			}
		})();

		return () => socket.end();
	}
}
