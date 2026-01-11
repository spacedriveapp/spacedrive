/**
 * Test setup for Bun integration tests
 * Provides a DOM environment for React Testing Library using Happy DOM
 *
 * Happy DOM is faster and lighter than jsdom, optimized for testing.
 * This one-liner registers all DOM globals automatically.
 */

import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register();

// Suppress React act() warnings for async event-driven state updates
// In integration tests, daemon events trigger React state updates asynchronously,
// which is expected behavior and doesn't need act() wrapping
const originalError = console.error;
console.error = (...args: any[]) => {
	const message = args[0];
	if (
		typeof message === "string" &&
		message.includes(
			"An update to TestComponent inside a test was not wrapped in act",
		)
	) {
		// Suppress act() warnings - they're expected for real-time event updates
		return;
	}
	originalError.apply(console, args);
};
