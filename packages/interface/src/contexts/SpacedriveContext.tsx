/**
 * Convenience re-exports from @sd/ts-client
 *
 * You can import from here OR directly from @sd/ts-client/hooks
 * Both work identically!
 */

// Export client type
// Export commonly used types for convenience
export type {
  LibraryInfo,
  Location,
  LocationsListOutput,
  SpacedriveClient,
} from "@sd/ts-client";
// Export icon utilities
export { getDeviceIcon, getVolumeIcon } from "@sd/ts-client";
// Re-export hooks from @sd/ts-client (no longer duplicated!)
export {
  SpacedriveProvider,
  useClient,
  useCoreMutation,
  useCoreQuery,
  useLibraryMutation,
  useLibraryQuery,
  useNormalizedQuery,
  useSpacedriveClient,
} from "@sd/ts-client/hooks";
