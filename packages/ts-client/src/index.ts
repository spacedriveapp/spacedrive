/**
 * @sd/ts-client - Type-safe TypeScript client for Spacedrive
 *
 * This package provides a complete type-safe interface to the Spacedrive core,
 * automatically generated from the Rust core types using Specta.
 *
 * @example Basic Client Usage
 * ```typescript
 * import { SpacedriveClient } from '@sd/ts-client';
 *
 * // Create client (Tauri)
 * const client = SpacedriveClient.fromTauri(invoke, listen);
 *
 * // Direct API calls (type-safe!)
 * const libraries = await client.execute('query:libraries.list', {});
 * ```
 *
 * @example React Hooks Usage
 * ```typescript
 * import { SpacedriveProvider, useLibraryQuery, useCoreMutation } from '@sd/ts-client/hooks';
 *
 * function App() {
 *   return (
 *     <SpacedriveProvider client={client}>
 *       <FileExplorer />
 *     </SpacedriveProvider>
 *   );
 * }
 *
 * function FileExplorer() {
 *   const { data: files } = useLibraryQuery({
 *     type: 'files.directory_listing',
 *     input: { path: '/' }
 *   });
 *
 *   const createTag = useCoreMutation('tags.create');
 *
 *   return <div>{files?.entries.map(f => f.name)}</div>;
 * }
 * ```
 */

// Core client
export { SpacedriveClient } from "./client";
// Device and volume utilities
export * from "./deviceIcons";
// Event filtering utilities
export {
  DEFAULT_EVENT_SUBSCRIPTION,
  type EventVariant,
  NOISY_EVENTS,
} from "./event-filter";
// All auto-generated types
export * from "./generated/types";
// React hooks (requires @tanstack/react-query peer dependency)
export * from "./hooks";
// Zustand stores
export * from "./stores/sidebar";
export * from "./stores/sortPreferences";
export * from "./stores/syncPreferences";
export * from "./stores/viewPreferences";
export { SubscriptionManager } from "./subscriptionManager";
export type { Transport } from "./transport";
export { TauriTransport, UnixSocketTransport } from "./transport";
export * from "./volumeIcons";
