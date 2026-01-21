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

import type { ReactNativeTransport } from "./transport";
import type { Event } from "@sd/ts-client/src/generated/types";

interface EventFilter {
	library_id?: string;
	resource_type?: string;
	path_scope?: any;
	include_descendants?: boolean;
	event_types?: string[];
}

interface SubscriptionEntry {
	unsubscribe: () => void;
	listeners: Set<(event: Event) => void>;
	refCount: number;
	filter: EventFilter;
}

export class SubscriptionManager {
	private subscriptions = new Map<string, SubscriptionEntry>();
	private pendingSubscriptions = new Map<string, Promise<SubscriptionEntry>>();
	private transport: ReactNativeTransport;

	constructor(transport: ReactNativeTransport) {
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
	 * Check if an event matches the filter
	 * Since mobile transport doesn't support server-side filtering,
	 * we filter events client-side
	 */
	private matchesFilter(event: Event, filter: EventFilter): boolean {
		if (filter.event_types && filter.event_types.length > 0) {
			const eventType = typeof event === "string" ? event : Object.keys(event)[0];
			if (!filter.event_types.includes(eventType)) {
				return false;
			}
		}

		return true;
	}

	/**
	 * Subscribe to filtered events
	 * Reuses existing subscription if filter matches
	 * Handles concurrent subscription requests for the same filter
	 */
	async subscribe(
		filter: EventFilter,
		callback: (event: Event) => void,
	): Promise<() => void> {
		const key = this.getFilterKey(filter);

		let entry = this.subscriptions.get(key);
		if (entry) {
			entry.listeners.add(callback);
			entry.refCount++;
			return this.createCleanup(key, callback);
		}

		const pending = this.pendingSubscriptions.get(key);
		if (pending) {
			entry = await pending;
			entry.listeners.add(callback);
			entry.refCount++;
			return this.createCleanup(key, callback);
		}

		const subscriptionPromise = this.createSubscription(key, filter);
		this.pendingSubscriptions.set(key, subscriptionPromise);

		try {
			entry = await subscriptionPromise;
			entry.listeners.add(callback);
			entry.refCount++;
			return this.createCleanup(key, callback);
		} finally {
			this.pendingSubscriptions.delete(key);
		}
	}

	private async createSubscription(
		key: string,
		filter: EventFilter,
	): Promise<SubscriptionEntry> {
		const unsubscribe = await this.transport.subscribe((event) => {
			const currentEntry = this.subscriptions.get(key);
			if (currentEntry && this.matchesFilter(event, filter)) {
				currentEntry.listeners.forEach((listener) => listener(event));
			}
		});

		const entry: SubscriptionEntry = {
			unsubscribe,
			listeners: new Set(),
			refCount: 0,
			filter,
		};

		this.subscriptions.set(key, entry);
		return entry;
	}

	private createCleanup(
		key: string,
		callback: (event: Event) => void,
	): () => void {
		return () => {
			const currentEntry = this.subscriptions.get(key);
			if (!currentEntry) return;

			currentEntry.listeners.delete(callback);
			currentEntry.refCount--;

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
		this.subscriptions.forEach((entry) => entry.unsubscribe());
		this.subscriptions.clear();
	}
}
