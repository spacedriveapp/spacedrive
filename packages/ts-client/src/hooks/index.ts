// React hooks for Spacedrive client
// These hooks provide type-safe, auto-generated bindings to the Spacedrive API

export { SpacedriveProvider, useSpacedriveClient, useClient, queryClient } from "./useClient";
export { useCoreQuery, useLibraryQuery } from "./useQuery";
export { useCoreMutation, useLibraryMutation } from "./useMutation";
export { useNormalizedQuery } from "./useNormalizedQuery";
// Alias for backwards compatibility
export { useNormalizedQuery as useNormalizedCache } from "./useNormalizedQuery";
