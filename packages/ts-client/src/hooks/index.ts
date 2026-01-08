// React hooks for Spacedrive client
// These hooks provide type-safe, auto-generated bindings to the Spacedrive API

export {
  queryClient,
  SpacedriveProvider,
  useClient,
  useSpacedriveClient,
} from "./useClient";
export { useCoreMutation, useLibraryMutation } from "./useMutation";
// Alias for backwards compatibility
export {
  useNormalizedQuery,
  useNormalizedQuery as useNormalizedCache,
} from "./useNormalizedQuery";
export { useCoreQuery, useLibraryQuery } from "./useQuery";
