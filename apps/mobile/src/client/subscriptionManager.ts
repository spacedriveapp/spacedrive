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
	/** Flag to prevent double cleanup */
	cleaned: boolean;
}

export class SubscriptionManager {
	private subscriptions = new Map<string, SubscriptionEntry>();
	private pendingSubscriptions = new Map<string, Promise<SubscriptionEntry>>();
	private transport: ReactNativeTransport;
	/** Flag to prevent cleanup races during destruction */
	private isDestroying = false;

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
			// Check cleaned flag to prevent processing during cleanup
			if (currentEntry && !currentEntry.cleaned && this.matchesFilter(event, filter)) {
				currentEntry.listeners.forEach((listener) => listener(event));
			}
		});

		const entry: SubscriptionEntry = {
			unsubscribe,
			listeners: new Set(),
			refCount: 0,
			filter,
			cleaned: false,
		};

		this.subscriptions.set(key, entry);
		return entry;
	}

	private createCleanup(
		key: string,
		callback: (event: Event) => void,
	): () => void {
		let hasRun = false;

		return () => {
			// Guard against double cleanup
			if (hasRun) return;
			hasRun = true;

			// Don't cleanup during destruction - destroy() handles it
			if (this.isDestroying) return;

			const currentEntry = this.subscriptions.get(key);
			if (!currentEntry || currentEntry.cleaned) return;

			currentEntry.listeners.delete(callback);
			currentEntry.refCount--;

			if (currentEntry.refCount === 0) {
				// Mark as cleaned first to prevent race conditions
				currentEntry.cleaned = true;

				// Defer unsubscribe to next tick to allow pending operations to complete
				setTimeout(() => {
					// Double check the entry is still the one we expect
					const entry = this.subscriptions.get(key);
					if (entry === currentEntry) {
						entry.unsubscribe();
						this.subscriptions.delete(key);
					}
				}, 0);
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
		// Set flag to prevent individual cleanup handlers from running
		this.isDestroying = true;

		// Mark all entries as cleaned first
		this.subscriptions.forEach((entry) => {
			entry.cleaned = true;
		});

		// Then unsubscribe all
		this.subscriptions.forEach((entry) => {
			try {
				entry.unsubscribe();
			} catch (e) {
				console.warn("[SubscriptionManager] Error during unsubscribe:", e);
			}
		});

		this.subscriptions.clear();
		this.pendingSubscriptions.clear();

		// Reset flag in case manager is reused
		this.isDestroying = false;
	}
}
