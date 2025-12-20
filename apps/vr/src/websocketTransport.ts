import type { Transport } from "@sd/ts-client";

/**
 * WebSocket transport for connecting to Spacedrive daemon via proxy server
 */
export class WebSocketTransport implements Transport {
	private ws: WebSocket | null = null;
	private messageHandlers: Set<(event: any) => void> = new Set();
	private pendingRequests: Map<
		number,
		{ resolve: (value: any) => void; reject: (error: any) => void }
	> = new Map();
	private requestId = 0;
	private reconnectAttempts = 0;
	private maxReconnectAttempts = 5;

	constructor(private wsUrl: string) {}

	/**
	 * Ensure WebSocket connection is established
	 */
	private async ensureConnected(): Promise<void> {
		if (this.ws && this.ws.readyState === WebSocket.OPEN) {
			return;
		}

		return new Promise((resolve, reject) => {
			console.log(`[WebSocketTransport] Connecting to ${this.wsUrl}...`);
			console.log(
				`[WebSocketTransport] Current time:`,
				new Date().toISOString(),
			);

			try {
				this.ws = new WebSocket(this.wsUrl);
			} catch (err) {
				console.error(
					"[WebSocketTransport] Failed to create WebSocket:",
					err,
				);
				reject(err);
				return;
			}

			// Timeout for connection
			const timeout = setTimeout(() => {
				console.error(
					"[WebSocketTransport] Connection timeout after 10s",
				);
				if (this.ws) {
					this.ws.close();
				}
				reject(new Error("WebSocket connection timeout"));
			}, 10000);

			this.ws.onopen = () => {
				clearTimeout(timeout);
				console.log("[WebSocketTransport] ✅ Connected successfully!");
				this.reconnectAttempts = 0;
				resolve();
			};

			this.ws.onerror = (error) => {
				clearTimeout(timeout);
				console.error(
					"[WebSocketTransport] ❌ Connection error:",
					error,
				);
				console.error("[WebSocketTransport] Error type:", error.type);
				console.error("[WebSocketTransport] URL:", this.wsUrl);
				reject(new Error(`WebSocket connection failed: ${error.type}`));
			};

			this.ws.onmessage = (event) => {
				try {
					const data = JSON.parse(event.data);

					// Check if this is an event (events have "Event" field)
					if (data.Event) {
						console.log(
							"[WebSocketTransport] Received event:",
							data.Event,
						);
						// Broadcast to all event subscribers
						this.messageHandlers.forEach((handler) =>
							handler(data.Event),
						);
					}
					// Otherwise it's a response to a request
					// (handled by individual request handlers via addEventListener)
				} catch (err) {
					console.error(
						"[WebSocketTransport] Failed to parse message:",
						err,
						event.data,
					);
				}
			};

			this.ws.onclose = () => {
				console.log("[WebSocketTransport] Connection closed");
				this.ws = null;

				// Auto-reconnect if we have active subscriptions
				if (
					this.messageHandlers.size > 0 &&
					this.reconnectAttempts < this.maxReconnectAttempts
				) {
					this.reconnectAttempts++;
					console.log(
						`[WebSocketTransport] Reconnecting (attempt ${this.reconnectAttempts})...`,
					);
					setTimeout(
						() => this.ensureConnected(),
						1000 * this.reconnectAttempts,
					);
				}
			};
		});
	}

	async sendRequest(request: any): Promise<any> {
		await this.ensureConnected();

		if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
			throw new Error("WebSocket not connected");
		}

		return new Promise((resolve, reject) => {
			console.log(
				"[WebSocketTransport] Sending request:",
				JSON.stringify(request).substring(0, 200),
			);

			// Create a one-time message handler for this request
			const handleResponse = (event: MessageEvent) => {
				try {
					const data = JSON.parse(event.data);
					console.log(
						"[WebSocketTransport] Got response:",
						JSON.stringify(data).substring(0, 200),
					);

					// Remove this handler after receiving response
					this.ws?.removeEventListener("message", handleResponse);

					resolve(data);
				} catch (err) {
					console.error(
						"[WebSocketTransport] Failed to parse response:",
						err,
					);
					reject(err);
				}
			};

			// Add temporary handler for this specific request
			this.ws.addEventListener("message", handleResponse);

			// Send request without modification (daemon expects exact format)
			this.ws!.send(JSON.stringify(request));

			// Timeout after 30 seconds
			setTimeout(() => {
				this.ws?.removeEventListener("message", handleResponse);
				reject(new Error("Request timeout after 30s"));
			}, 30000);
		});
	}

	async subscribe(callback: (event: any) => void): Promise<() => void> {
		await this.ensureConnected();

		console.log("[WebSocketTransport] Subscribing to events");
		this.messageHandlers.add(callback);

		// Send subscribe request to daemon
		const subscribeRequest = {
			Subscribe: {
				event_types: [
					"LibraryCreated",
					"LibraryOpened",
					"LibraryClosed",
					"LibraryDeleted",
					"LibraryStatisticsUpdated",
				],
				filter: null,
			},
		};

		this.ws!.send(JSON.stringify(subscribeRequest));

		// Return unsubscribe function
		return () => {
			console.log("[WebSocketTransport] Unsubscribing");
			this.messageHandlers.delete(callback);

			// If no more subscribers, send Unsubscribe to daemon
			if (this.messageHandlers.size === 0 && this.ws) {
				this.ws.send(JSON.stringify("Unsubscribe"));
			}
		};
	}
}
