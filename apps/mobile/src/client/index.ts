// Core client
export { SpacedriveClient } from "./SpacedriveClient";
export { ReactNativeTransport } from "./transport";

// Provider and hooks
export { SpacedriveProvider, useSpacedriveClient } from "./hooks/useClient";
export {
	useCoreQuery,
	useLibraryQuery,
	useCoreAction,
	useLibraryAction,
} from "./hooks/useQuery";

// Re-export shared hooks from ts-client
export { useNormalizedQuery } from "@sd/ts-client/src/hooks/useNormalizedQuery";
