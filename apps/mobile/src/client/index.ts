// Core client

// Re-export shared hooks from ts-client
export { useNormalizedQuery } from "@sd/ts-client/src/hooks/useNormalizedQuery";
// Provider and hooks
export { SpacedriveProvider, useSpacedriveClient } from "./hooks/useClient";
export {
  useCoreAction,
  useCoreQuery,
  useLibraryAction,
  useLibraryQuery,
} from "./hooks/useQuery";
export { SpacedriveClient } from "./SpacedriveClient";
export { ReactNativeTransport } from "./transport";
