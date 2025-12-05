import React, {
	createContext,
	useContext,
	useEffect,
	useState,
	useMemo,
	ReactNode,
} from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SpacedriveClient } from "../SpacedriveClient";
import { View, Text, ActivityIndicator, StyleSheet } from "react-native";
import AsyncStorage from "@react-native-async-storage/async-storage";

// Context for the Spacedrive client
const ClientContext = createContext<SpacedriveClient | null>(null);

/**
 * Hook to access the Spacedrive client.
 * Must be used within a SpacedriveProvider.
 */
export function useSpacedriveClient(): SpacedriveClient {
	const client = useContext(ClientContext);
	if (!client) {
		throw new Error(
			"useSpacedriveClient must be used within SpacedriveProvider",
		);
	}
	return client;
}

// Create a stable QueryClient instance
const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			staleTime: 30 * 1000, // 30 seconds
			retry: 2,
		},
	},
});

interface SpacedriveProviderProps {
	children: ReactNode;
	deviceName?: string;
}

/**
 * Provider component that initializes the Spacedrive core
 * and provides the client context to children.
 */
export function SpacedriveProvider({
	children,
	deviceName,
}: SpacedriveProviderProps) {
	const [client] = useState(() => new SpacedriveClient());
	const [initialized, setInitialized] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		let mounted = true;

		async function init() {
			try {
				await client.initialize(deviceName ?? "Spacedrive Mobile");

				// Load persisted library ID from storage
				const storedData = await AsyncStorage.getItem("spacedrive-sidebar");
				let libraryIdSet = false;

				if (storedData) {
					const parsed = JSON.parse(storedData);
					if (parsed.state?.currentLibraryId) {
						console.log("[SpacedriveProvider] Restoring library ID:", parsed.state.currentLibraryId);
						client.setCurrentLibrary(parsed.state.currentLibraryId);
						libraryIdSet = true;
					}
				}

				// If no library ID was restored, try to auto-select the first library
				if (!libraryIdSet) {
					try {
						const libraries = await client.coreQuery("libraries.list", { include_stats: false });
						if (libraries && Array.isArray(libraries) && libraries.length > 0) {
							const firstLibrary = libraries[0];
							console.log("[SpacedriveProvider] Auto-selecting first library:", firstLibrary.name, firstLibrary.id);
							client.setCurrentLibrary(firstLibrary.id);

							// Also save to AsyncStorage for next time
							await AsyncStorage.setItem(
								"spacedrive-sidebar",
								JSON.stringify({
									state: {
										currentLibraryId: firstLibrary.id,
										collapsedGroups: [],
									},
								})
							);
						} else {
							console.warn("[SpacedriveProvider] No libraries available to auto-select");
						}
					} catch (error) {
						console.error("[SpacedriveProvider] Failed to auto-select library:", error);
					}
				}

				if (mounted) {
					setInitialized(true);
				}
			} catch (e) {
				console.error("[SpacedriveProvider] Failed to initialize:", e);
				if (mounted) {
					setError(
						e instanceof Error ? e.message : "Failed to initialize",
					);
				}
			}
		}

		init();

		return () => {
			mounted = false;
			client.destroy();
		};
	}, [client, deviceName]);

	if (error) {
		return (
			<View style={styles.container}>
				<Text style={styles.errorTitle}>Initialization Error</Text>
				<Text style={styles.errorText}>{error}</Text>
			</View>
		);
	}

	if (!initialized) {
		return (
			<View style={styles.container}>
				<ActivityIndicator size="large" color="#2599FF" />
				<Text style={styles.loadingText}>
					Initializing Spacedrive...
				</Text>
			</View>
		);
	}

	return (
		<ClientContext.Provider value={client}>
			<QueryClientProvider client={queryClient}>
				{children}
			</QueryClientProvider>
		</ClientContext.Provider>
	);
}

const styles = StyleSheet.create({
	container: {
		flex: 1,
		backgroundColor: "hsl(235, 15%, 13%)",
		alignItems: "center",
		justifyContent: "center",
		padding: 20,
	},
	loadingText: {
		color: "hsl(235, 10%, 70%)",
		marginTop: 16,
		fontSize: 16,
	},
	errorTitle: {
		color: "#ff5555",
		fontSize: 20,
		fontWeight: "bold",
		marginBottom: 8,
	},
	errorText: {
		color: "hsl(235, 10%, 70%)",
		fontSize: 14,
		textAlign: "center",
	},
});
