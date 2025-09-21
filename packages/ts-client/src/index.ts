/**
 * Spacedrive TypeScript Client
 *
 * A type-safe TypeScript client for interacting with the Spacedrive daemon.
 */

export { SpacedriveClient, SpacedriveError, SpacedriveClientExamples } from './client';
export { Transport, UnixSocketTransport } from './transport';

// Re-export generated types when they become available
// export * from './types';

// Version information
export const VERSION = '0.1.0';
