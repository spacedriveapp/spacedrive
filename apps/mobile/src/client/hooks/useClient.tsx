import React, { useEffect, useState, ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import {
  SpacedriveClientContext,
  queryClient,
  useSpacedriveClient,
} from "@sd/ts-client/src/hooks/useClient";
import type { Event } from "@sd/ts-client/src/generated/types";
import { SpacedriveClient } from "../SpacedriveClient";
import { View, Text, ActivityIndicator, StyleSheet } from "react-native";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { SDMobileCore } from "sd-mobile-core";
import { usePreferencesStore } from "../../stores/preferences";
import { useSidebarStore } from "../../stores/sidebar";
import { useReactQueryDevTools } from "@dev-plugins/react-query";

// Re-export the shared hook
export { useSpacedriveClient };

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

  // Initialize React Query DevTools (Expo plugin) - must be at top before any returns
  useReactQueryDevTools(queryClient);

  useEffect(() => {
    let mounted = true;
    let unsubscribeLogs: (() => void) | null = null;

    async function init() {
      try {
        await client.initialize(deviceName ?? "Spacedrive Mobile");

        // // Subscribe to core logs AFTER core is initialized
        // console.log("[SpacedriveProvider] Subscribing to core logs...");
        // unsubscribeLogs = SDMobileCore.addLogListener((log) => {
        // 	console.log("[SpacedriveProvider] RAW LOG RECEIVED:", log);
        // 	try {
        // 		const logData = JSON.parse(log.body);
        // 		console.log(`[CORE ${logData.level}] ${logData.target}: ${logData.message}`);
        // 	} catch (e) {
        // 		console.error("[SpacedriveProvider] Failed to parse log:", log.body);
        // 	}
        // });
        // console.log("[SpacedriveProvider] Log listener subscribed");

        // Load persisted library ID from storage and validate it exists
        const storedData = await AsyncStorage.getItem("spacedrive-sidebar");
        let storedLibraryId: string | null = null;

        if (storedData) {
          const parsed = JSON.parse(storedData);
          storedLibraryId = parsed.state?.currentLibraryId || null;
        }

        // Fetch available libraries to validate the stored library ID
        try {
          const libraries = await client.coreQuery("libraries.list", {
            include_stats: false,
          });

          if (libraries && Array.isArray(libraries) && libraries.length > 0) {
            // Check if stored library ID exists in the list
            const storedLibraryExists =
              storedLibraryId &&
              libraries.some((lib) => lib.id === storedLibraryId);

            if (storedLibraryExists) {
              console.log(
                "[SpacedriveProvider] Restoring library ID:",
                storedLibraryId,
              );
              client.setCurrentLibrary(storedLibraryId);
            } else {
              // Stored library doesn't exist, auto-select first library
              const firstLibrary = libraries[0];
              console.log(
                storedLibraryId
                  ? "[SpacedriveProvider] Stored library no longer exists, auto-selecting first library:"
                  : "[SpacedriveProvider] Auto-selecting first library:",
                firstLibrary.name,
                firstLibrary.id,
              );
              client.setCurrentLibrary(firstLibrary.id);

              // Save to AsyncStorage for next time
              await AsyncStorage.setItem(
                "spacedrive-sidebar",
                JSON.stringify({
                  state: {
                    currentLibraryId: firstLibrary.id,
                    collapsedGroups: [],
                  },
                }),
              );
            }
          } else {
            console.warn(
              "[SpacedriveProvider] No libraries available to auto-select",
            );
          }
        } catch (error) {
          console.error(
            "[SpacedriveProvider] Failed to fetch/validate libraries:",
            error,
          );
        }

        // Subscribe to core events for auto-switching on synced library creation
        const unsubscribeEvents = await client.subscribe((event: Event) => {
          // Check if this is a LibraryCreated event from sync
          if (
            typeof event === "object" &&
            "LibraryCreated" in event &&
            (event as any).LibraryCreated.source === "Sync"
          ) {
            const { id, name } = (event as any).LibraryCreated;

            // Check user preference for auto-switching
            const autoSwitchEnabled =
              usePreferencesStore.getState().autoSwitchOnSync;

            if (autoSwitchEnabled) {
              console.log(
                `[Auto-Switch] Received synced library "${name}", switching...`,
              );

              // Update client state
              client.setCurrentLibrary(id);

              // Update sidebar store (persisted to AsyncStorage)
              useSidebarStore.getState().setCurrentLibrary(id);
            } else {
              console.log(
                `[Auto-Switch] Received synced library "${name}", but auto-switch is disabled`,
              );
            }
          }
        });

        if (mounted) {
          setInitialized(true);
        }

        // Store unsubscribe for cleanup
        return unsubscribeEvents;
      } catch (e) {
        console.error("[SpacedriveProvider] Failed to initialize:", e);
        if (mounted) {
          setError(e instanceof Error ? e.message : "Failed to initialize");
        }
        return null;
      }
    }

    const initPromise = init();

    return () => {
      mounted = false;
      if (unsubscribeLogs) unsubscribeLogs();
      initPromise.then((unsubscribe) => {
        if (unsubscribe) unsubscribe();
      });
      client.destroy();
      // Clear query cache on unmount for clean reset
      queryClient.clear();
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
        <Text style={styles.loadingText}>Initializing Spacedrive...</Text>
      </View>
    );
  }

  return (
    <QueryClientProvider client={queryClient}>
      <SpacedriveClientContext.Provider value={client}>
        {children}
      </SpacedriveClientContext.Provider>
    </QueryClientProvider>
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
