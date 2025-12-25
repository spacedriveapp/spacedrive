/**
 * Platform-agnostic transport layer for Spacedrive client
 * Uses Bun APIs for Unix sockets and Tauri invoke for browser
 */

import { DEFAULT_EVENT_SUBSCRIPTION } from "./event-filter";
import type { SdPath } from "./types";

export interface EventFilter {
	library_id?: string;
	job_id?: string;
	device_id?: string;
	resource_type?: string;
	path_scope?: SdPath;
	include_descendants?: boolean;
}

export interface SubscriptionOptions {
	event_types?: string[];
	filter?: EventFilter;
}

export interface Transport {
	sendRequest(request: any): Promise<any>;
	subscribe(
		callback: (event: any) => void,
		options?: SubscriptionOptions,
	): Promise<() => void>;
}

/**
 * Tauri transport using IPC invoke
 * This works in the browser (Tauri webview)
 */
export class TauriTransport implements Transport {
	private invoke: (cmd: string, args?: any) => Promise<any>;
	private listen: (
		event: string,
		handler: (event: any) => void,
	) => Promise<() => void>;

	constructor(
		invoke: (cmd: string, args?: any) => Promise<any>,
		listen: (
			event: string,
			handler: (event: any) => void,
		) => Promise<() => void>,
	) {
		this.invoke = invoke;
		this.listen = listen;
	}

	async sendRequest(request: any): Promise<any> {
		const response = await this.invoke("daemon_request", { request });
		return response;
	}

	async subscribe(
		callback: (event: any) => void,
		options?: SubscriptionOptions,
	): Promise<() => void> {
		// Start the event subscription on the backend
		// Returns subscription ID for cleanup
		const args = {
			eventTypes: options?.event_types ?? DEFAULT_EVENT_SUBSCRIPTION,
			filter: options?.filter ?? null,
		};
		const subscriptionId = await this.invoke("subscribe_to_events", args);

		// Listen to forwarded events from Tauri
		const unlisten = await this.listen("core-event", (tauriEvent: any) => {
			callback(tauriEvent.payload);
		});

		// Return cleanup function that properly unsubscribes
		return async () => {
			unlisten();
			try {
				await this.invoke("unsubscribe_from_events", {
					subscriptionId,
				});
			} catch (e) {
				console.warn("[TauriTransport] Failed to unsubscribe:", e);
			}
		};
	}
}

/**
 * TCP socket transport for Bun/Node environments
 * Connects to daemon via TCP (e.g., "127.0.0.1:6969")
 */
export class TcpSocketTransport implements Transport {
	constructor(private socketAddr: string) {}

	async sendRequest(request: any): Promise<any> {
		// Parse socket address (e.g., "127.0.0.1:6969")
		const [hostname, portStr] = this.socketAddr.split(":");
		const port = parseInt(portStr, 10);

		return new Promise((resolve, reject) => {
			let buffer = "";

			// @ts-ignore - Bun global
			Bun.connect({
				hostname,
				port,
				socket: {
					data(socket: any, data: any) {
						buffer += new TextDecoder().decode(data);

						const newlineIndex = buffer.indexOf("\n");
						if (newlineIndex !== -1) {
							const line = buffer.slice(0, newlineIndex).trim();
							socket.end();
							try {
								resolve(JSON.parse(line));
							} catch (e) {
								reject(e);
							}
						}
					},
					open(socket: any) {
						const requestLine = JSON.stringify(request) + "\n";
						socket.write(requestLine);
					},
					error(socket: any, error: Error) {
						reject(error);
					},
					close(socket: any) {
						if (buffer && !buffer.includes("\n")) {
							reject(new Error("Connection closed without complete response"));
						}
					},
				},
			});
		});
	}

	async subscribe(
		callback: (event: any) => void,
		options?: SubscriptionOptions,
	): Promise<() => void> {
		// Parse socket address
		const [hostname, portStr] = this.socketAddr.split(":");
		const port = parseInt(portStr, 10);

		let socketInstance: any = null;
		let buffer = "";

		// Subscribe to relevant events
		const subscribeRequest = {
			Subscribe: {
				event_types: options?.event_types ?? DEFAULT_EVENT_SUBSCRIPTION,
				filter: options?.filter ?? null,
			},
		};

		// @ts-ignore - Bun global
		socketInstance = await Bun.connect({
			hostname,
			port,
			socket: {
				data(socket: any, data: any) {
					buffer += new TextDecoder().decode(data);

					let newlineIndex: number;
					while ((newlineIndex = buffer.indexOf("\n")) !== -1) {
						const line = buffer.slice(0, newlineIndex).trim();
						buffer = buffer.slice(newlineIndex + 1);

						if (line) {
							try {
								const response = JSON.parse(line);

								// Handle DaemonResponse variants
								if (response === "Subscribed" || response.Subscribed !== undefined) {
									// Subscription acknowledgment, don't forward
								} else if (response.Event) {
									// Event message, forward to callback
									callback(response.Event);
								} else if (response.LogMessage) {
									// Log message, forward to callback
									callback(response.LogMessage);
								} else {
									console.warn(
										"[TcpSocketTransport] Unexpected response:",
										response,
									);
								}
							} catch (e) {
								console.error("[TcpSocketTransport] Parse error:", e);
							}
						}
					}
				},
				open(socket: any) {
					// Send subscription request once connected
					socket.write(JSON.stringify(subscribeRequest) + "\n");
				},
				error(socket: any, error: Error) {
					console.error("[TcpSocketTransport] Socket error:", error);
				},
				close(socket: any) {
					console.log("[TcpSocketTransport] Connection closed");
				},
			},
		});

		// Return unsubscribe function
		return () => {
			if (socketInstance) {
				socketInstance.end();
			}
		};
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

	async subscribe(
		callback: (event: any) => void,
		options?: SubscriptionOptions,
	): Promise<() => void> {
		// @ts-ignore - Bun global
		const socket = await Bun.connect({
			unix: this.socketPath,
		});

		// Subscribe to relevant events (excludes spammy LogMessage/JobProgress)
		const subscribeRequest = {
			Subscribe: {
				event_types: options?.event_types ?? DEFAULT_EVENT_SUBSCRIPTION,
				filter: options?.filter ?? null,
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
