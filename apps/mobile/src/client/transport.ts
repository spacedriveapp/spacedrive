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

export interface JsonRpcResponse {
	jsonrpc: "2.0";
	id: string;
	result?: unknown;
	error?: { code: number; message: string };
}

type PendingRequest = {
	resolve: (result: unknown) => void;
	reject: (error: Error) => void;
};

let requestCounter = 0;

/**
 * Transport layer for communicating with the embedded Spacedrive core.
 * Batches requests for efficiency and handles JSON-RPC protocol.
 */
export class ReactNativeTransport {
	private pendingRequests = new Map<string, PendingRequest>();
	private batch: JsonRpcRequest[] = [];
	private batchQueued = false;

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
			pending.reject(new Error(response.error.message));
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
			this.batchQueued = false;

			if (currentBatch.length === 0) return;

			try {
				const query = JSON.stringify(
					currentBatch.length === 1 ? currentBatch[0] : currentBatch,
				);
				const resultStr = await SDMobileCore.sendMessage(query);
				const result = JSON.parse(resultStr);

				if (Array.isArray(result)) {
					result.forEach(this.processResponse);
				} else {
					this.processResponse(result);
				}
			} catch (e) {
				console.error("[Transport] ❌ Batch request failed:", e);
				for (const req of currentBatch) {
					const pending = this.pendingRequests.get(req.id);
					if (pending) {
						pending.reject(new Error("Batch request failed"));
						this.pendingRequests.delete(req.id);
					}
				}
			}
		}, 0);
	}

	/**
	 * Send a request to the core and return a promise with the result.
	 */
	async request<T = unknown>(
		method: string,
		params: { input: unknown; library_id?: string },
	): Promise<T> {
		return new Promise((resolve, reject) => {
			const id = `${++requestCounter}`;

			this.pendingRequests.set(id, {
				resolve: resolve as (result: unknown) => void,
				reject,
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
	 * Clean up resources.
	 */
	destroy() {
		this.pendingRequests.clear();
		this.batch = [];
	}
}
