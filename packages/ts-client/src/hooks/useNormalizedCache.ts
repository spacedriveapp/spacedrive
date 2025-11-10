import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useSpacedriveClient } from "./useClient";

interface UseNormalizedCacheOptions<I> {
	/** Wire method to call (e.g., "query:locations.list") */
	wireMethod: string;
	/** Input for the query */
	input: I;
	/** Resource type for cache indexing (e.g., "location") */
	resourceType: string;
	/** Whether the query is enabled (default: true) */
	enabled?: boolean;
}

/**
 * React hook that wraps TanStack Query with event-driven cache updates
 *
 * This hook:
 * 1. Uses TanStack Query normally (all refetching behavior preserved)
 * 2. Listens for ResourceChanged events for the given resource type
 * 3. When event arrives, atomically updates TanStack Query's cache
 * 4. Component re-renders instantly with new data
 *
 * TanStack Query continues to refetch based on its normal rules (staleTime, etc.),
 * but events provide instant updates without waiting for refetch.
 *
 * Example:
 * ```tsx
 * const { data: locations, isLoading } = useNormalizedCache({
 *   wireMethod: 'query:locations.list',
 *   input: {},
 *   resourceType: 'location',
 * });
 *
 * // When LocationA is created on Device B:
 * // 1. Backend emits ResourceChanged event
 * // 2. Event listener updates TanStack Query cache atomically
 * // 3. This component re-renders
 * // 4. User sees new location instantly!
 * // 5. TanStack Query may refetch in background (normal behavior)
 * ```
 */
export function useNormalizedCache<I, O>({
	wireMethod,
	input,
	resourceType,
	enabled = true,
}: UseNormalizedCacheOptions<I>) {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();

	// Get current library ID for library-scoped queries
	const libraryId = client.getCurrentLibraryId();

	// Include library ID in key so switching libraries triggers refetch
	const queryKey = [wireMethod, libraryId, input];

	// Use TanStack Query normally
	const query = useQuery<O>({
		queryKey,
		queryFn: async () => {
			// Client.execute() automatically adds library_id to the request
			// as a sibling field to payload (not inside it!)
			console.log("useNormalizedCache queryFn:", {
				wireMethod,
				input,
				libraryId,
				resourceType,
			});

			try {
				const result = await client.execute<I, O>(wireMethod, input);
				console.log("useNormalizedCache result:", result);
				return result;
			} catch (error) {
				console.error("useNormalizedCache error:", error);
				throw error;
			}
		},
		enabled: enabled && !!libraryId,
	});

	// Listen for ResourceChanged events and update cache atomically
	useEffect(() => {
		const handleEvent = (event: any) => {
			console.log("useNormalizedCache - Event received:", { event, resourceType });

			// Check if this is a ResourceChanged event for our resource type
			if ("ResourceChanged" in event) {
				const { resource_type, resource } = event.ResourceChanged;

				console.log("useNormalizedCache - ResourceChanged:", {
					resource_type,
					matchesOurType: resource_type === resourceType,
					resource,
				});

				if (resource_type === resourceType) {
					console.log("useNormalizedCache - Updating cache for:", { queryKey, resource });

					// Atomic update: merge this resource into the query data
					queryClient.setQueryData<O>(queryKey, (oldData) => {
						console.log("useNormalizedCache - setQueryData oldData:", oldData);

						if (!oldData) return oldData;

						// Handle both array responses and wrapped responses
						// e.g., LocationsListOutput = { locations: LocationInfo[] }
						if (Array.isArray(oldData)) {
							// Direct array response
							const resourceId = resource.id;
							const existingIndex = oldData.findIndex((item: any) => item.id === resourceId);

							if (existingIndex >= 0) {
								const newData = [...oldData];
								newData[existingIndex] = resource;
								return newData as O;
							} else {
								return [...oldData, resource] as O;
							}
						} else if (oldData && typeof oldData === 'object') {
							// Wrapped response - look for array field
							// Try common wrapper field names
							const arrayField = Object.keys(oldData).find(
								key => Array.isArray((oldData as any)[key])
							);

							if (arrayField) {
								const array = (oldData as any)[arrayField];
								const resourceId = resource.id;
								const existingIndex = array.findIndex((item: any) => item.id === resourceId);

								if (existingIndex >= 0) {
									const newArray = [...array];
									newArray[existingIndex] = resource;
									return { ...oldData, [arrayField]: newArray };
								} else {
									return { ...oldData, [arrayField]: [...array, resource] };
								}
							}
						}

						return oldData;
					});
				}
			} else if ("ResourceDeleted" in event) {
				const { resource_type, resource_id } = event.ResourceDeleted;

				if (resource_type === resourceType) {
					// Atomic update: remove deleted resource
					queryClient.setQueryData<O>(queryKey, (oldData) => {
						if (!oldData) return oldData;

						if (Array.isArray(oldData)) {
							return oldData.filter((item: any) => item.id !== resource_id) as O;
						} else if (oldData && typeof oldData === 'object') {
							const arrayField = Object.keys(oldData).find(
								key => Array.isArray((oldData as any)[key])
							);

							if (arrayField) {
								const array = (oldData as any)[arrayField];
								return {
									...oldData,
									[arrayField]: array.filter((item: any) => item.id !== resource_id)
								};
							}
						}

						return oldData;
					});
				}
			}
		};

		// Subscribe to events
		const unsubscribe = client.on("spacedrive-event", handleEvent);

		return () => {
			client.off("spacedrive-event", handleEvent);
		};
	}, [resourceType, queryKey, queryClient]);

	return query;
}
