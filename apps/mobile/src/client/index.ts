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

// Helper to get properly typed mobile client
import type { SpacedriveClient as MobileClient } from "./SpacedriveClient";
import { useSpacedriveClient as _useClient } from "./hooks/useClient";
export function useMobileClient(): MobileClient {
	return _useClient() as unknown as MobileClient;
}

// Re-export shared hooks from ts-client
export { useNormalizedQuery } from "@sd/ts-client/src/hooks/useNormalizedQuery";
export { useSearchFiles } from "@sd/ts-client";
