import { createContext, useContext, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { SpacedriveClient } from "../client";

// Export context so platforms can provide their own wrappers
export const SpacedriveClientContext = createContext<SpacedriveClient | null>(null);

// Create a singleton query client
export const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			staleTime: 30000, // 30 seconds
			gcTime: 300000, // 5 minutes
			retry: 1,
			refetchOnWindowFocus: true,
			refetchOnReconnect: true,
		},
	},
});

export interface SpacedriveProviderProps {
	client: SpacedriveClient;
	children: ReactNode;
}

/**
 * Provider for SpacedriveClient + TanStack Query
 * Wrap your app with this to make the client available via hooks
 */
export function SpacedriveProvider({ client, children }: SpacedriveProviderProps) {
	return (
		<QueryClientProvider client={queryClient}>
			<SpacedriveClientContext.Provider value={client}>
				{children}
			</SpacedriveClientContext.Provider>
		</QueryClientProvider>
	);
}

/**
 * Hook to access the Spacedrive client
 * Must be used within a SpacedriveProvider
 */
export function useSpacedriveClient(): SpacedriveClient {
	const client = useContext(SpacedriveClientContext);

	if (!client) {
		throw new Error("useSpacedriveClient must be used within SpacedriveProvider");
	}

	return client;
}

// Also export for direct use
export { useClient };
function useClient() {
	return useSpacedriveClient();
}
