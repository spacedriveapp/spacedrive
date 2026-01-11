import type { Transport } from "./transport";
import { UnixSocketTransport, TcpSocketTransport, TauriTransport } from "./transport";
import type { Event } from "./generated/types";
import { DEFAULT_EVENT_SUBSCRIPTION } from "./event-filter";
import { SubscriptionManager } from "./subscriptionManager";

/**
 * Simple event emitter for browser compatibility
 */
class SimpleEventEmitter {
	private listeners: Map<string, Set<Function>> = new Map();

	on(event: string, listener: Function) {
		if (!this.listeners.has(event)) {
			this.listeners.set(event, new Set());
		}
		this.listeners.get(event)!.add(listener);
	}

	emit(event: string, ...args: any[]) {
		const listeners = this.listeners.get(event);
		if (listeners) {
			listeners.forEach((listener) => listener(...args));
		}
	}

	once(event: string, listener: Function) {
		const onceWrapper = (...args: any[]) => {
			listener(...args);
			this.off(event, onceWrapper);
		};
		this.on(event, onceWrapper);
	}

	off(event: string, listener: Function) {
		const listeners = this.listeners.get(event);
		if (listeners) {
			listeners.delete(listener);
		}
	}
}

/**
 * Type-safe TypeScript client for Spacedrive
 *
 * This client mirrors the Swift client architecture with:
 * - Namespaced APIs (libraries, files, jobs, etc.)
 * - Type-safe operations using generated types
 * - Library context management
 * - Event subscription system
 */
export class SpacedriveClient extends SimpleEventEmitter {
	private transport: Transport;
	private currentLibraryId: string | null = null;
	private subscriptionManager: SubscriptionManager;

	constructor(transport: Transport) {
		super();
		this.transport = transport;
		this.subscriptionManager = new SubscriptionManager(transport);
	}

	/**
	 * Create client for Bun/Node.js using Unix socket
	 */
	static fromSocket(socketPath: string): SpacedriveClient {
		return new SpacedriveClient(new UnixSocketTransport(socketPath));
	}

	/**
	 * Create client for Bun/Node.js using TCP socket
	 * @param socketAddr - TCP address (e.g., "127.0.0.1:6969")
	 */
	static fromTcpSocket(socketAddr: string): SpacedriveClient {
		return new SpacedriveClient(new TcpSocketTransport(socketAddr));
	}

	/**
	 * Create client for Tauri using IPC
	 */
	static fromTauri(
		invoke: (cmd: string, args?: any) => Promise<any>,
		listen: (
			event: string,
			handler: (event: any) => void,
		) => Promise<() => void>,
	): SpacedriveClient {
		const client = new SpacedriveClient(new TauriTransport(invoke, listen));
		client.setupEventLogging();
		return client;
	}

	/**
	 * Setup global event logging (logs each event once)
	 */
	private setupEventLogging() {
		// Event logging removed for production - enable in debug mode if needed
	}

	// MARK: - Library Context Management

	/**
	 * Get the currently active library ID
	 */
	getCurrentLibraryId(): string | null {
		return this.currentLibraryId;
	}

	/**
	 * Set the currently active library
	 * @param emitEvent - Whether to emit library-changed event (default: true). Set to false when already triggered by external event.
	 */
	setCurrentLibrary(libraryId: string, emitEvent: boolean = true): void {
		this.currentLibraryId = libraryId;

		// Emit a library-changed event that hooks can listen to (unless already triggered externally)
		if (emitEvent) {
			this.emit("library-changed", libraryId);
		}
	}

	/**
	 * Clear the currently active library
	 */
	clearCurrentLibrary(): void {
		this.currentLibraryId = null;
	}

	/**
	 * Switch to a library by ID
	 * Verifies the library exists before switching
	 */
	async switchToLibrary(libraryId: string): Promise<void> {
		// Verify library exists by calling the query directly
		const libraries = await this.execute<{}, any[]>(
			"query:libraries.list",
			{},
		);
		const libraryExists = libraries.some(
			(lib: any) => lib.id === libraryId,
		);

		if (!libraryExists) {
			throw new Error(`Library with ID '${libraryId}' not found`);
		}

		this.setCurrentLibrary(libraryId);
	}

	/**
	 * Get information about the currently active library
	 */
	async getCurrentLibraryInfo() {
		const libraryId = this.getCurrentLibraryId();
		if (!libraryId) return null;

		const libraries = await this.execute<{}, any[]>(
			"query:libraries.list",
			{},
		);
		return libraries.find((lib: any) => lib.id === libraryId) ?? null;
	}

	/**
	 * Require a current library or throw
	 * @internal
	 */
	requireCurrentLibrary(): string {
		const libraryId = this.getCurrentLibraryId();
		if (!libraryId) {
			throw new Error(
				"This operation requires an active library. Use switchToLibrary() first.",
			);
		}
		return libraryId;
	}

	// MARK: - Core Execution Methods

	/**
	 * Execute a wire method with the given input
	 * This is the low-level method used by TanStack Query hooks
	 */
	async execute<I, O>(wireMethod: string, input: I): Promise<O> {
		// Determine if this is a query or action based on wire method prefix
		const isQuery = wireMethod.startsWith("query:");
		const isAction = wireMethod.startsWith("action:");

		if (!isQuery && !isAction) {
			throw new Error(`Invalid wire method: ${wireMethod}`);
		}

		const request = isQuery
			? {
					Query: {
						method: wireMethod,
						library_id: this.currentLibraryId, // ← Sibling field!
						payload: input,
					},
				}
			: {
					Action: {
						method: wireMethod,
						library_id: this.currentLibraryId, // ← Sibling field!
						payload: input,
					},
				};

		const response = await this.transport.sendRequest(request);

		// Handle different response formats
		if ("JsonOk" in response) {
			return response.JsonOk;
		} else if ("json" in response) {
			// Wire protocol uses lowercase "json" for success
			return response.json;
		} else if ("Error" in response || "error" in response) {
			const error = response.Error || response.error;
			throw new Error(
				`${isQuery ? "Query" : "Action"} failed: ${JSON.stringify(error)}`,
			);
		} else {
			throw new Error(`Unexpected response: ${JSON.stringify(response)}`);
		}
	}

	/**
	 * Subscribe to events from the daemon
	 */
	async subscribe(callback?: (event: Event) => void): Promise<() => void> {
		const unlisten = await this.transport.subscribe((event) => {
			if (callback) {
				callback(event);
			}
		});

		return unlisten;
	}

	/**
	 * Subscribe to filtered events from the daemon
	 * Uses subscription manager to multiplex connections
	 */
	async subscribeFiltered(
		filter: {
			resource_type?: string;
			path_scope?: import("./types").SdPath;
			library_id?: string;
			include_descendants?: boolean;
			event_types?: string[];
		},
		callback: (event: Event) => void,
	): Promise<() => void> {
		return this.subscriptionManager.subscribe(filter, callback);
	}

	/**
	 * Get subscription manager stats for debugging
	 */
	getSubscriptionStats() {
		return this.subscriptionManager.getStats();
	}

	/**
	 * Ping the daemon to test connectivity
	 */
	async ping(): Promise<void> {
		const response = await this.transport.sendRequest("Ping");

		if (response === "Pong") {
			console.log("Ping successful!");
		} else {
			throw new Error(
				`Unexpected ping response: ${JSON.stringify(response)}`,
			);
		}
	}
}

// Export all types for convenience
export * from "./generated/types";
