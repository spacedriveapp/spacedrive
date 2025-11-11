import type { Transport } from "./transport";
import { UnixSocketTransport, TauriTransport } from "./transport";
import type { Event } from "./generated/types";

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

	constructor(transport: Transport) {
		super();
		this.transport = transport;
	}

	/**
	 * Create client for Bun/Node.js using Unix socket
	 */
	static fromSocket(socketPath: string): SpacedriveClient {
		return new SpacedriveClient(new UnixSocketTransport(socketPath));
	}

	/**
	 * Create client for Tauri using IPC
	 */
	static fromTauri(
		invoke: (cmd: string, args?: any) => Promise<any>,
		listen: (event: string, handler: (event: any) => void) => Promise<() => void>
	): SpacedriveClient {
		return new SpacedriveClient(new TauriTransport(invoke, listen));
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
	 */
	setCurrentLibrary(libraryId: string): void {
		this.currentLibraryId = libraryId;
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
		const libraries = await this.execute<{}, any[]>("query:libraries.list", {});
		const libraryExists = libraries.some((lib: any) => lib.id === libraryId);

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

		const libraries = await this.execute<{}, any[]>("query:libraries.list", {});
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
				"This operation requires an active library. Use switchToLibrary() first."
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
						library_id: this.currentLibraryId,  // ← Sibling field!
						payload: input,
					},
			  }
			: {
					Action: {
						method: wireMethod,
						library_id: this.currentLibraryId,  // ← Sibling field!
						payload: input,
					},
			  };

		console.log("SpacedriveClient.execute - Sending request:", {
			wireMethod,
			input,
			currentLibraryId: this.currentLibraryId,
			request,
		});

		const response = await this.transport.sendRequest(request);

		console.log("SpacedriveClient.execute - Received response:", response);

		// Handle different response formats
		if ("JsonOk" in response) {
			// Debug directory listing responses
			if (wireMethod === "query:files.directory_listing" && response.JsonOk?.files) {
				const fileWithContent = response.JsonOk.files.find((f: any) => f.content_identity);
				if (fileWithContent) {
					console.log("Directory listing - file with content:", {
						name: fileWithContent.name,
						hasContentIdentity: !!fileWithContent.content_identity,
						contentIdentity: fileWithContent.content_identity,
						hasSidecars: !!fileWithContent.sidecars,
						sidecarCount: fileWithContent.sidecars?.length,
						sidecars: fileWithContent.sidecars
					});
				}
			}
			return response.JsonOk;
		} else if ("json" in response) {
			// Wire protocol uses lowercase "json" for success
			return response.json;
		} else if ("Error" in response || "error" in response) {
			const error = response.Error || response.error;
			throw new Error(
				`${isQuery ? "Query" : "Action"} failed: ${JSON.stringify(error)}`
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
			// Log ALL events to debug ResourceChanged issue
			const eventType = Object.keys(event)[0];

			if ("ResourceChanged" in event) {
				console.log("ResourceChanged:", event.ResourceChanged.resource_type, event.ResourceChanged);
			} else if ("ResourceDeleted" in event) {
				console.log("️ ResourceDeleted:", event.ResourceDeleted.resource_type);
			} else if ("JobProgress" in event) {
				// Don't log progress events, too spammy
			} else {
				console.log("SpacedriveClient - Event:", eventType);
			}

			// Emit to SimpleEventEmitter (useNormalizedCache listens to this)
			this.emit("spacedrive-event", event);

			if (callback) {
				callback(event);
			}
		});

		return unlisten;
	}

	/**
	 * Ping the daemon to test connectivity
	 */
	async ping(): Promise<void> {
		const response = await this.transport.sendRequest("Ping");

		if (response === "Pong") {
			console.log("Ping successful!");
		} else {
			throw new Error(`Unexpected ping response: ${JSON.stringify(response)}`);
		}
	}

}

// Export all types for convenience
export * from "./generated/types";
