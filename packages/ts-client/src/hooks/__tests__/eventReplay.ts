/**
 * Event Replay Test Utilities
 *
 * Simulates backend event streams for testing normalized query cache updates.
 * Uses real backend event data from fixtures for accurate testing.
 */

import type { Event } from "../../generated/types";

export class EventReplaySimulator {
  private events: Event[];
  private eventIndex = 0;
  private speed = 0; // 0 = instant, >0 = delay in ms

  constructor(events: Event[], speed = 0) {
    this.events = events;
    this.speed = speed;
  }

  async replayNext(callback: (event: Event) => void): Promise<boolean> {
    if (this.eventIndex >= this.events.length) {
      return false; // No more events
    }

    const event = this.events[this.eventIndex++];

    if (this.speed > 0) {
      await new Promise((resolve) => setTimeout(resolve, this.speed));
    }

    callback(event);
    return true;
  }

  async replayAll(callback: (event: Event) => void): Promise<void> {
    while (await this.replayNext(callback)) {
      // Continue until all events replayed
    }
  }

  reset() {
    this.eventIndex = 0;
  }

  getProgress() {
    return {
      current: this.eventIndex,
      total: this.events.length,
      remaining: this.events.length - this.eventIndex,
    };
  }
}

/**
 * Create a mock SpacedriveClient for testing
 */
export function createMockClient(initialData: any) {
  const subscriptions = new Map<number, (event: Event) => void>();
  const libraryChangeHandlers = new Set<Function>();
  let subscriptionId = 0;
  let currentLibraryId = "test-library-id";

  const client = {
    execute: async (wireMethod: string, input: any) => {
      // Return initial query response
      return initialData;
    },
    subscribeFiltered: async (
      filter: any,
      callback: (event: Event) => void
    ) => {
      // Store the callback
      const id = subscriptionId++;
      subscriptions.set(id, callback);
      console.log("[MockClient] Subscription created:", id, "filter:", filter);

      // Return unsubscribe function
      return () => {
        subscriptions.delete(id);
        console.log("[MockClient] Subscription removed:", id);
      };
    },
    getCurrentLibraryId: () => currentLibraryId,
    setCurrentLibrary: (libraryId: string) => {
      currentLibraryId = libraryId;
      libraryChangeHandlers.forEach((h) => h(libraryId));
    },
    on: (event: string, handler: Function) => {
      if (event === "library-changed") {
        libraryChangeHandlers.add(handler);
      }
    },
    off: (event: string, handler: Function) => {
      if (event === "library-changed") {
        libraryChangeHandlers.delete(handler);
      }
    },
    // Expose subscriptions for testing
    __testOnly_triggerEvent: (event: Event) => {
      console.log(
        "[MockClient] Triggering event to",
        subscriptions.size,
        "subscribers"
      );
      subscriptions.forEach((callback, id) => {
        console.log("[MockClient] Calling subscriber", id);
        callback(event);
      });
    },
    __testOnly_getSubscriptionCount: () => subscriptions.size,
  };

  return client;
}
