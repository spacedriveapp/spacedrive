import { SDMobileCore, CoreEvent } from "sd-mobile-core";
import type { Event } from "@sd/ts-client/src/generated/types";

export interface EventFilter {
	library_id?: string;
	job_id?: string;
	device_id?: string;
	resource_type?: string;
	path_scope?: any;
	include_descendants?: boolean;
}

export interface SubscriptionOptions {
	event_types?: string[];
	filter?: EventFilter;
}

export interface JsonRpcRequest {
	jsonrpc: "2.0";
	id: string;
	method: string;
	params: {
		input: unknown;
		library_id?: string;
	};
}

export interface JsonRpcErrorData {
	error_type: string;
	details?: Record<string, unknown>;
}

export interface JsonRpcResponse {
	jsonrpc: "2.0";
	id: string;
	result?: unknown;
	error?: { code: number; message: string; data?: JsonRpcErrorData };
}

/**
 * Custom error class for Spacedrive errors with additional context.
 */
export class SpacedriveError extends Error {
	public readonly code: number;
	public readonly errorType: string;
	public readonly details?: Record<string, unknown>;

	constructor(
		message: string,
		code: number,
		errorType: string,
		details?: Record<string, unknown>,
	) {
		super(message);
		this.name = "SpacedriveError";
		this.code = code;
		this.errorType = errorType;
		this.details = details;
	}

	/**
	 * Check if this is a specific error type.
	 */
	isType(errorType: string): boolean {
		return this.errorType === errorType;
	}
}

type PendingRequest = {
	resolve: (result: unknown) => void;
	reject: (error: Error) => void;
	timeoutId?: ReturnType<typeof setTimeout>;
};

let requestCounter = 0;

// Timeout configuration
const DEFAULT_TIMEOUT_MS = 30000; // 30 seconds for normal requests
const LONG_RUNNING_TIMEOUT_MS = 120000; // 2 minutes for long-running operations

// Methods that are known to take longer
const LONG_RUNNING_METHODS = [
	"action:locations.add",
	"action:locations.rescan",
	"action:libraries.create",
	"action:jobs.run",
];

// Retry configuration
export interface RetryConfig {
	maxRetries: number;
	baseDelayMs: number;
	maxDelayMs: number;
	backoffMultiplier: number;
}

const DEFAULT_RETRY_CONFIG: RetryConfig = {
	maxRetries: 3,
	baseDelayMs: 1000,
	maxDelayMs: 10000,
	backoffMultiplier: 2,
};

// Errors that should not be retried
const NON_RETRYABLE_ERROR_TYPES = [
	"INVALID_REQUEST", // Client error - request is malformed
	"INVALID_METHOD", // Client error - method doesn't exist
	"INVALID_LIBRARY_ID", // Client error - bad library ID format
	"LIBRARY_NOT_FOUND", // Client error - library doesn't exist
	"SECURITY_ERROR", // Security violation - shouldn't retry
	"VALIDATION_ERROR", // Client error - invalid input
];

/**
 * Check if an error should be retried.
 */
function isRetryableError(error: Error): boolean {
	if (error instanceof SpacedriveError) {
		return !NON_RETRYABLE_ERROR_TYPES.includes(error.errorType);
	}
	// Retry network errors and unknown errors
	return true;
}

/**
 * Calculate delay for exponential backoff with jitter.
 */
function calculateRetryDelay(attempt: number, config: RetryConfig): number {
	const exponentialDelay =
		config.baseDelayMs * Math.pow(config.backoffMultiplier, attempt);
	const boundedDelay = Math.min(exponentialDelay, config.maxDelayMs);
	// Add jitter (10-20% randomization)
	const jitter = boundedDelay * (0.1 + Math.random() * 0.1);
	return boundedDelay + jitter;
}

/**
 * Sleep for the specified duration.
 */
function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Check if a method is a long-running operation.
 */
function isLongRunningMethod(method: string): boolean {
	return LONG_RUNNING_METHODS.some((m) => method.startsWith(m));
}

// Health check configuration
const HEALTH_CHECK_INTERVAL_MS = 30000; // 30 seconds
const HEALTH_CHECK_TIMEOUT_MS = 5000; // 5 seconds

// Batch processing configuration
const BATCH_TIMEOUT_BASE_MS = 30000; // Base timeout for batch
const BATCH_TIMEOUT_PER_REQUEST_MS = 5000; // Additional timeout per request in batch

export type HealthStatus = "healthy" | "unhealthy" | "unknown";

export interface HealthCheckResult {
	status: HealthStatus;
	latencyMs?: number;
	error?: string;
}

/**
 * Transport layer for communicating with the embedded Spacedrive core.
 * Batches requests for efficiency and handles JSON-RPC protocol.
 */
export class ReactNativeTransport {
	private pendingRequests = new Map<string, PendingRequest>();
	private batch: JsonRpcRequest[] = [];
	private batchQueued = false;
	private healthCheckInterval: ReturnType<typeof setInterval> | null = null;
	private currentHealthStatus: HealthStatus = "unknown";
	private healthListeners = new Set<(result: HealthCheckResult) => void>();

	constructor() {
		// No event listener needed - responses come through sendMessage promise
	}

	private processResponse = (response: JsonRpcResponse) => {
		const pending = this.pendingRequests.get(response.id);
		if (!pending) {
			return;
		}

		if (response.error) {
			console.error("[Transport] ❌ Response error:", response.error);
			const errorData = response.error.data;
			const error = new SpacedriveError(
				response.error.message,
				response.error.code,
				errorData?.error_type ?? "UNKNOWN_ERROR",
				errorData?.details,
			);
			pending.reject(error);
		} else {
			pending.resolve(response.result);
		}

		this.pendingRequests.delete(response.id);
	};

	private queueBatch() {
		if (this.batchQueued) return;
		this.batchQueued = true;

		// Use setImmediate-like behavior for batching
		setTimeout(async () => {
			const currentBatch = [...this.batch];
			this.batch = [];

			// Reset batchQueued after taking the batch, not before processing
			// This prevents new requests from being lost if processing fails

			if (currentBatch.length === 0) {
				this.batchQueued = false;
				return;
			}

			// Calculate batch timeout based on number of requests
			const batchTimeout =
				BATCH_TIMEOUT_BASE_MS + currentBatch.length * BATCH_TIMEOUT_PER_REQUEST_MS;

			try {
				const query = JSON.stringify(
					currentBatch.length === 1 ? currentBatch[0] : currentBatch,
				);

				// Create timeout promise for batch-level timeout
				const timeoutPromise = new Promise<never>((_, reject) => {
					setTimeout(() => {
						reject(
							new SpacedriveError(
								`Batch request timeout after ${batchTimeout}ms (${currentBatch.length} requests)`,
								-32000,
								"BATCH_TIMEOUT",
								{ requestCount: currentBatch.length, timeout: batchTimeout },
							),
						);
					}, batchTimeout);
				});

				// Race between actual request and timeout
				const resultStr = await Promise.race([
					SDMobileCore.sendMessage(query),
					timeoutPromise,
				]);
				const result = JSON.parse(resultStr);

				if (Array.isArray(result)) {
					result.forEach(this.processResponse);
				} else {
					this.processResponse(result);
				}
			} catch (e) {
				console.error("[Transport] Batch request failed:", e);

				// Determine error type for better error messages
				const errorType =
					e instanceof SpacedriveError ? e.errorType : "BATCH_FAILED";
				const errorMessage =
					e instanceof Error ? e.message : "Batch request failed";

				// Reject all pending requests in the batch with specific error
				for (const req of currentBatch) {
					const pending = this.pendingRequests.get(req.id);
					if (pending) {
						const error = new SpacedriveError(
							`${errorMessage} (method: ${req.method})`,
							-32000,
							errorType,
							{ method: req.method },
						);
						pending.reject(error);
						this.pendingRequests.delete(req.id);
					}
				}
			} finally {
				// Always reset batchQueued in finally to ensure recovery
				this.batchQueued = false;
			}
		}, 0);
	}

	/**
	 * Send a request to the core and return a promise with the result.
	 * @param method The JSON-RPC method to call
	 * @param params The parameters for the method
	 * @param options Optional configuration including custom timeout and retry config
	 */
	async request<T = unknown>(
		method: string,
		params: { input: unknown; library_id?: string },
		options?: { timeout?: number; retry?: Partial<RetryConfig> | false },
	): Promise<T> {
		const retryConfig =
			options?.retry === false
				? null
				: { ...DEFAULT_RETRY_CONFIG, ...options?.retry };

		let lastError: Error | null = null;
		const maxAttempts = retryConfig ? retryConfig.maxRetries + 1 : 1;

		for (let attempt = 0; attempt < maxAttempts; attempt++) {
			try {
				return await this.requestInternal<T>(method, params, options?.timeout);
			} catch (error) {
				lastError = error instanceof Error ? error : new Error(String(error));

				// Check if we should retry
				const isLastAttempt = attempt >= maxAttempts - 1;
				const shouldRetry = retryConfig && !isLastAttempt && isRetryableError(lastError);

				if (!shouldRetry) {
					throw lastError;
				}

				// Calculate and wait for retry delay
				const delay = calculateRetryDelay(attempt, retryConfig);
				console.warn(
					`[Transport] Request failed, retrying in ${Math.round(delay)}ms (attempt ${attempt + 1}/${maxAttempts}): ${method}`,
				);
				await sleep(delay);
			}
		}

		// Should never reach here, but TypeScript needs this
		throw lastError ?? new Error("Request failed");
	}

	/**
	 * Internal request implementation without retry logic.
	 */
	private requestInternal<T = unknown>(
		method: string,
		params: { input: unknown; library_id?: string },
		timeout?: number,
	): Promise<T> {
		return new Promise((resolve, reject) => {
			const id = `${++requestCounter}`;

			// Determine timeout based on method type or explicit option
			const effectiveTimeout =
				timeout ??
				(isLongRunningMethod(method) ? LONG_RUNNING_TIMEOUT_MS : DEFAULT_TIMEOUT_MS);

			// Set up timeout handler
			const timeoutId = setTimeout(() => {
				const pending = this.pendingRequests.get(id);
				if (pending) {
					this.pendingRequests.delete(id);
					console.error(`[Transport] Request timeout after ${effectiveTimeout}ms: ${method}`);
					reject(
						new SpacedriveError(
							`Request timeout after ${effectiveTimeout}ms: ${method}`,
							-32000,
							"TIMEOUT",
							{ method, timeout: effectiveTimeout },
						),
					);
				}
			}, effectiveTimeout);

			this.pendingRequests.set(id, {
				resolve: (result: unknown) => {
					clearTimeout(timeoutId);
					resolve(result as T);
				},
				reject: (error: Error) => {
					clearTimeout(timeoutId);
					reject(error);
				},
				timeoutId,
			});

			this.batch.push({
				jsonrpc: "2.0",
				id,
				method,
				params,
			});

			this.queueBatch();
		});
	}

	/**
	 * Subscribe to events from the embedded core.
	 * Note: Mobile core doesn't support per-subscription filtering yet.
	 * All filtering happens client-side via SubscriptionManager.
	 */
	async subscribe(
		callback: (event: Event) => void,
		_options?: SubscriptionOptions,
	): Promise<() => void> {
		const unlisten = SDMobileCore.addListener((coreEvent: CoreEvent) => {
			try {
				const event = JSON.parse(coreEvent.body) as Event;
				callback(event);
			} catch (e) {
				console.error("[Transport] ❌ Failed to parse event:", e);
			}
		});

		return unlisten;
	}

	/**
	 * Start periodic health checks.
	 * @param intervalMs Interval between health checks (default: 30 seconds)
	 */
	startHealthCheck(intervalMs: number = HEALTH_CHECK_INTERVAL_MS): void {
		if (this.healthCheckInterval) {
			return; // Already running
		}

		// Run initial check
		this.performHealthCheck();

		// Schedule periodic checks
		this.healthCheckInterval = setInterval(() => {
			this.performHealthCheck();
		}, intervalMs);
	}

	/**
	 * Stop periodic health checks.
	 */
	stopHealthCheck(): void {
		if (this.healthCheckInterval) {
			clearInterval(this.healthCheckInterval);
			this.healthCheckInterval = null;
		}
	}

	/**
	 * Add a listener for health status changes.
	 * @returns Cleanup function to remove the listener
	 */
	onHealthChange(listener: (result: HealthCheckResult) => void): () => void {
		this.healthListeners.add(listener);
		return () => {
			this.healthListeners.delete(listener);
		};
	}

	/**
	 * Get the current health status.
	 */
	getHealthStatus(): HealthStatus {
		return this.currentHealthStatus;
	}

	/**
	 * Perform a single health check.
	 */
	async performHealthCheck(): Promise<HealthCheckResult> {
		const startTime = Date.now();

		try {
			// Use a simple query that should always succeed
			await this.requestInternal<unknown>(
				"query:core.ping",
				{ input: {} },
				HEALTH_CHECK_TIMEOUT_MS,
			);

			const latencyMs = Date.now() - startTime;
			const result: HealthCheckResult = {
				status: "healthy",
				latencyMs,
			};

			this.updateHealthStatus(result);
			return result;
		} catch (error) {
			const result: HealthCheckResult = {
				status: "unhealthy",
				error: error instanceof Error ? error.message : String(error),
			};

			this.updateHealthStatus(result);
			return result;
		}
	}

	private updateHealthStatus(result: HealthCheckResult): void {
		const previousStatus = this.currentHealthStatus;
		this.currentHealthStatus = result.status;

		// Only notify listeners if status changed or if unhealthy (always report errors)
		if (previousStatus !== result.status || result.status === "unhealthy") {
			this.healthListeners.forEach((listener) => {
				try {
					listener(result);
				} catch (e) {
					console.warn("[Transport] Health listener error:", e);
				}
			});
		}
	}

	/**
	 * Clean up resources including pending timeouts.
	 */
	destroy() {
		// Stop health checks
		this.stopHealthCheck();

		// Clear all pending timeouts before clearing the map
		for (const pending of this.pendingRequests.values()) {
			if (pending.timeoutId) {
				clearTimeout(pending.timeoutId);
			}
		}
		this.pendingRequests.clear();
		this.batch = [];
		this.healthListeners.clear();
	}
}
