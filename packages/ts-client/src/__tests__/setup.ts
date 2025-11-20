/**
 * Test setup configuration
 */

import '@testing-library/jest-dom';

// Suppress console errors during tests
global.console = {
	...console,
	error: jest.fn(),
	warn: jest.fn(),
};

