/**
 * Test setup for Bun test runner
 * Provides DOM environment for React Testing Library
 */

import { JSDOM } from "jsdom";

const dom = new JSDOM("<!DOCTYPE html><html><body></body></html>", {
  url: "http://localhost",
});

global.document = dom.window.document;
global.window = dom.window as any;
global.navigator = dom.window.navigator;
