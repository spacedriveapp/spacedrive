import { SDMobileCore } from "sd-mobile-core";
import {
	ReactNativeTransport,
	type HealthCheckResult,
	type HealthStatus,
} from "./transport";
import { WIRE_METHODS } from "@sd/ts-client";
import type { Event } from "@sd/ts-client/generated/types";
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
 * Spacedrive client for React Native.
 * Manages the embedded core lifecycle and provides query/mutation methods.
 */
export class SpacedriveClient extends SimpleEventEmitter {
  private transport: ReactNativeTransport;
  private currentLibraryId: string | null = null;
  private initialized = false;
  private subscriptionManager: SubscriptionManager;

  constructor() {
    super();
    this.transport = new ReactNativeTransport();
    this.subscriptionManager = new SubscriptionManager(this.transport);
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
   * @param emitEvent - Whether to emit library-changed event (default: true)
   */
  setCurrentLibrary(libraryId: string | null, emitEvent: boolean = true) {
    this.currentLibraryId = libraryId;

    if (emitEvent && libraryId) {
      this.emit("library-changed", libraryId);
    }
  }

  /**
   * Get the current library ID.
   */
  getCurrentLibraryId(): string | null {
    return this.currentLibraryId;
  }

  /**
   * Execute a wire method directly (used by useNormalizedQuery)
   * Matches the desktop client's execute method signature
   */
  async execute<I, O>(wireMethod: string, input: I): Promise<O> {
    const isQuery = wireMethod.startsWith("query:");
    const isAction = wireMethod.startsWith("action:");

    if (!isQuery && !isAction) {
      throw new Error(`Invalid wire method: ${wireMethod}`);
    }

    return this.transport.request<O>(wireMethod, {
      input,
      library_id: this.currentLibraryId ?? undefined,
    });
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
      path_scope?: any;
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
   * Start connection health monitoring.
   * Health checks run periodically and emit 'connection-health' events.
   * @param intervalMs Interval between checks (default: 30 seconds)
   */
  startHealthMonitoring(intervalMs?: number): void {
    this.transport.startHealthCheck(intervalMs);
  }

  /**
   * Stop connection health monitoring.
   */
  stopHealthMonitoring(): void {
    this.transport.stopHealthCheck();
  }

  /**
   * Get the current connection health status.
   */
  getHealthStatus(): HealthStatus {
    return this.transport.getHealthStatus();
  }

  /**
   * Add a listener for connection health changes.
   * @returns Cleanup function to remove the listener
   */
  onHealthChange(listener: (result: HealthCheckResult) => void): () => void {
    const cleanup = this.transport.onHealthChange((result) => {
      listener(result);
      // Also emit as event for compatibility
      this.emit("connection-health", result);
    });
    return cleanup;
  }

  /**
   * Perform a single health check and return the result.
   */
  async checkHealth(): Promise<HealthCheckResult> {
    return this.transport.performHealthCheck();
  }

  /**
   * Shutdown the core and clean up resources.
   */
  destroy() {
    this.stopHealthMonitoring();
    this.subscriptionManager.destroy();
    this.transport.destroy();
    SDMobileCore.shutdown();
    this.initialized = false;
  }
}
