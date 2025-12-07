/**
 * Convenience re-exports from @sd/ts-client
 *
 * You can import from here OR directly from @sd/ts-client/hooks
 * Both work identically!
 */

// Re-export hooks from @sd/ts-client (no longer duplicated!)
export {
	SpacedriveProvider,
	useSpacedriveClient,
	useClient,
	useCoreQuery,
	useLibraryQuery,
	useCoreMutation,
	useLibraryMutation,
	useNormalizedQuery,
} from "@sd/ts-client/hooks";

// Export client type
export type { SpacedriveClient } from "@sd/ts-client";

// Export commonly used types for convenience
export type {
	LocationInfo,
	LocationsListOutput,
	LibraryInfo,
} from "@sd/ts-client";

// Export icon utilities
export { getDeviceIcon, getVolumeIcon } from "@sd/ts-client";