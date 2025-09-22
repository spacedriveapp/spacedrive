/**
 * @spacedrive/client - Type-safe TypeScript client for Spacedrive daemon
 *
 * This package provides a complete type-safe interface to the Spacedrive daemon,
 * automatically generated from the Rust core types using Specta.
 *
 * @example
 * ```typescript
 * import { SpacedriveClient } from '@spacedrive/client';
 *
 * const client = new SpacedriveClient();
 *
 * // Type-safe API calls
 * const libraries = await client.getLibraries();
 * const jobs = await client.getJobs();
 *
 * // Type-safe event subscription
 * await client.subscribe(['JobStarted', 'JobProgress', 'JobCompleted']);
 * client.on('event', (event) => {
 *   // event is fully typed as Event union
 *   console.log('Received event:', event);
 * });
 * ```
 */

export { SpacedriveClient } from './client';
export * from './types';
