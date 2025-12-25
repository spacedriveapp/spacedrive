/**
 * Test setup for Bun integration tests
 * Provides a DOM environment for React Testing Library using Happy DOM
 *
 * Happy DOM is faster and lighter than jsdom, optimized for testing.
 * This one-liner registers all DOM globals automatically.
 */

import { GlobalRegistrator } from "@happy-dom/global-registrator";

GlobalRegistrator.register();
