import { SDMobileCore } from "sd-mobile-core";
import { ReactNativeTransport } from "./transport";
import { WIRE_METHODS } from "@sd/ts-client";

/**
 * Spacedrive client for React Native.
 * Manages the embedded core lifecycle and provides query/mutation methods.
 */
export class SpacedriveClient {
	private transport: ReactNativeTransport;
	private currentLibraryId: string | null = null;
	private initialized = false;

	constructor() {
		this.transport = new ReactNativeTransport();
	}

	/**
	 * Initialize the embedded Spacedrive core.
	 * @param deviceName Optional device name for identification
	 */
	async initialize(deviceName?: string): Promise<void> {
		if (this.initialized) return;

		const result = await SDMobileCore.initialize(undefined, deviceName);
		if (result !== 0) {
			throw new Error(`Failed to initialize core: error code ${result}`);
		}

		this.initialized = true;
	}

	/**
	 * Check if the core is initialized.
	 */
	isInitialized(): boolean {
		return this.initialized;
	}

	/**
	 * Set the current library context for queries.
	 */
	setCurrentLibrary(libraryId: string | null) {
		this.currentLibraryId = libraryId;
	}

	/**
	 * Get the current library ID.
	 */
	getCurrentLibraryId(): string | null {
		return this.currentLibraryId;
	}

	/**
	 * Execute a core-level query (no library context required).
	 */
	async coreQuery<T = unknown>(
		method: string,
		input: unknown = {},
	): Promise<T> {
		const wireMethod = (WIRE_METHODS.coreQueries as any)[method];
		if (!wireMethod) {
			throw new Error(`Unknown core query: ${method}`);
		}
		return this.transport.request<T>(wireMethod, { input });
	}

	/**
	 * Execute a library-level query.
	 */
	async libraryQuery<T = unknown>(
		method: string,
		input: unknown = {},
	): Promise<T> {
		if (!this.currentLibraryId) {
			throw new Error("No library selected");
		}

		const wireMethod = (WIRE_METHODS.libraryQueries as any)[method];
		if (!wireMethod) {
			throw new Error(`Unknown library query: ${method}`);
		}

		return this.transport.request<T>(wireMethod, {
			input,
			library_id: this.currentLibraryId,
		});
	}

	/**
	 * Execute a core-level action (mutation).
	 */
	async coreAction<T = unknown>(
		method: string,
		input: unknown = {},
	): Promise<T> {
		const wireMethod = (WIRE_METHODS.coreActions as any)[method];
		if (!wireMethod) {
			throw new Error(`Unknown core action: ${method}`);
		}
		return this.transport.request<T>(wireMethod, { input });
	}

	/**
	 * Execute a library-level action (mutation).
	 */
	async libraryAction<T = unknown>(
		method: string,
		input: unknown = {},
	): Promise<T> {
		if (!this.currentLibraryId) {
			throw new Error("No library selected");
		}

		const wireMethod = (WIRE_METHODS.libraryActions as any)[method];
		if (!wireMethod) {
			throw new Error(`Unknown library action: ${method}`);
		}

		return this.transport.request<T>(wireMethod, {
			input,
			library_id: this.currentLibraryId,
		});
	}

	/**
	 * Shutdown the core and clean up resources.
	 */
	destroy() {
		this.transport.destroy();
		SDMobileCore.shutdown();
		this.initialized = false;
	}
}
