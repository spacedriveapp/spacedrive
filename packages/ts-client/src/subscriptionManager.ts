/**
 * Subscription Manager - Multiplexes event subscriptions
 *
 * Problem: Each useNormalizedQuery creates its own subscription, causing:
 * - Hundreds of subscriptions during render cycles
 * - Rapid subscribe/unsubscribe churn
 * - Multiple subscriptions with identical filters
 *
 * Solution: Pool subscriptions by filter signature:
 * - One backend subscription serves many hooks
 * - Reference counting prevents premature cleanup
 * - Automatic cleanup when last listener unsubscribes
 */

import type { Transport } from "./transport";
import type { Event } from "./generated/types";

interface EventFilter {
	library_id?: string;
	resource_type?: string;
	path_scope?: any;
	include_descendants?: boolean;
	event_types?: string[];
}

interface SubscriptionEntry {
	/** Cleanup function from transport */
	unsubscribe: () => void;
	/** All listeners for this subscription */
	listeners: Set<(event: Event) => void>;
	/** Reference count (number of hooks using this) */
	refCount: number;
}

export class SubscriptionManager {
	private subscriptions = new Map<string, SubscriptionEntry>();
	private transport: Transport;

	constructor(transport: Transport) {
		this.transport = transport;
	}

	/**
	 * Generate stable key for filter
	 * Same filters = same key = shared subscription
	 */
	private getFilterKey(filter: EventFilter): string {
		return JSON.stringify({
			library_id: filter.library_id ?? null,
			resource_type: filter.resource_type ?? null,
			path_scope: filter.path_scope ?? null,
			include_descendants: filter.include_descendants ?? false,
			event_types: filter.event_types ?? [],
		});
	}

	/**
	 * Subscribe to filtered events
	 * Reuses existing subscription if filter matches
	 */
	async subscribe(
		filter: EventFilter,
		callback: (event: Event) => void,
	): Promise<() => void> {
		const key = this.getFilterKey(filter);
		let entry = this.subscriptions.get(key);

		// Create new subscription if needed
		if (!entry) {
			const eventTypes = filter.event_types ?? [
				"ResourceChanged",
				"ResourceChangedBatch",
				"ResourceDeleted",
				"Refresh",
			];

			const unsubscribe = await this.transport.subscribe(
				(event) => {
					// Broadcast event to all listeners
					const currentEntry = this.subscriptions.get(key);
					if (currentEntry) {
						currentEntry.listeners.forEach((listener) => listener(event));
					}
				},
				{
					event_types: eventTypes,
					filter: {
						resource_type: filter.resource_type,
						path_scope: filter.path_scope,
						library_id: filter.library_id,
						include_descendants: filter.include_descendants,
					},
				},
			);

			entry = {
				unsubscribe,
				listeners: new Set(),
				refCount: 0,
			};

			this.subscriptions.set(key, entry);
		}

		// Add listener and increment ref count
		entry.listeners.add(callback);
		entry.refCount++;

		// Return cleanup function
		return () => {
			const currentEntry = this.subscriptions.get(key);
			if (!currentEntry) return;

			// Remove listener and decrement ref count
			currentEntry.listeners.delete(callback);
			currentEntry.refCount--;


			// Cleanup subscription if no more listeners
			if (currentEntry.refCount === 0) {
				currentEntry.unsubscribe();
				this.subscriptions.delete(key);
			}
		};
	}

	/**
	 * Get stats for debugging
	 */
	getStats() {
		return {
			activeSubscriptions: this.subscriptions.size,
			subscriptions: Array.from(this.subscriptions.entries()).map(
				([key, entry]) => ({
					key,
					refCount: entry.refCount,
					listenerCount: entry.listeners.size,
				}),
			),
		};
	}

	/**
	 * Force cleanup all subscriptions (for testing/cleanup)
	 */
	destroy() {
		console.log(
			`[SubscriptionManager] Destroying ${this.subscriptions.size} subscriptions`,
		);
		this.subscriptions.forEach((entry) => entry.unsubscribe());
		this.subscriptions.clear();
	}
}
