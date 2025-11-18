import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useSpacedriveClient } from "./useClient";

/**
 * Deep merge that preserves existing non-null values
 * Uses metadata from Identifiable trait to determine merge behavior
 *
 * @param existing - The current cached value
 * @param incoming - The new value from the event
 * @param noMergeFields - Fields to replace (from Identifiable.no_merge_fields)
 */
function deepMerge(
	existing: any,
	incoming: any,
	noMergeFields: string[] = [],
): any {
	// If incoming is null/undefined, keep existing
	if (incoming === null || incoming === undefined) {
		return existing !== null && existing !== undefined
			? existing
			: incoming;
	}

	// If types don't match or not objects, incoming wins
	if (
		typeof existing !== "object" ||
		typeof incoming !== "object" ||
		Array.isArray(existing) ||
		Array.isArray(incoming)
	) {
		return incoming;
	}

	// Both are objects - deep merge
	const merged: any = { ...incoming };

	for (const key in existing) {
		// Check if this field should not be merged (from backend Identifiable trait)
		if (noMergeFields.includes(key)) {
			continue; // Use incoming value as-is
		}

		if (!(key in incoming)) {
			// Key exists in old but not new - preserve it
			merged[key] = existing[key];
		} else if (incoming[key] === null || incoming[key] === undefined) {
			// Key exists in both but new is null - preserve old
			if (existing[key] !== null && existing[key] !== undefined) {
				merged[key] = existing[key];
			}
		} else if (
			typeof existing[key] === "object" &&
			typeof incoming[key] === "object" &&
			!Array.isArray(existing[key]) &&
			!Array.isArray(incoming[key])
		) {
			// Both are objects - recurse
			merged[key] = deepMerge(
				existing[key],
				incoming[key],
				noMergeFields,
			);
		}
		// else: incoming wins (has non-null value)
	}

	return merged;
}

/**
 * Check if a resource matches by ID or alternate IDs
 * Uses metadata from Identifiable trait for matching
 */
function resourceMatches(
	existing: any,
	incoming: any,
	alternateIds: string[] = [],
): boolean {
	// Match by primary ID
	if (existing.id === incoming.id) {
		return true;
	}

	// Match by any alternate ID (e.g., content UUID for Files)
	for (const altId of alternateIds) {
		if (existing.id === altId || incoming.id === altId) {
			return true;
		}
	}

	return false;
}

interface UseNormalizedCacheOptions<I> {
	/** Wire method to call (e.g., "query:locations.list") */
	wireMethod: string;
	/** Input for the query */
	input: I;
	/** Resource type for cache indexing (e.g., "location") */
	resourceType: string;
	/** Whether the query is enabled (default: true) */
	enabled?: boolean;
	/**
	 * Optional filter function to check if a resource belongs in this query.
	 * If not provided, all resources that pass the pathScope filter will be added (global list behavior).
	 * Use this for additional filtering beyond path scope (e.g., file type, tags, etc.)
	 */
	resourceFilter?: (resource: any) => boolean;
	/** Resource ID for single-resource queries (filters events to matching ID only) */
	resourceId?: string;
	/**
	 * Optional path scope for filtering events to a specific directory/path.
	 * When provided, the backend includes affected_paths in event metadata for efficient filtering.
	 *
	 * Note: Full server-side filtering is available via EventFilter.path_scope in the daemon,
	 * but current client architecture uses a single global subscription. Future enhancement
	 * could create separate filtered subscriptions per hook.
	 */
	pathScope?: import("../types").SdPath;
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
	resourceFilter,
	resourceId,
	pathScope,
}: UseNormalizedCacheOptions<I>) {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();

	// Track library ID reactively so queryKey updates when it changes
	const [libraryId, setLibraryId] = useState<string | null>(
		client.getCurrentLibraryId(),
	);

	// Listen for library ID changes and update our state (causes re-render)
	useEffect(() => {
		const handleLibraryChange = (newLibraryId: string) => {
			setLibraryId(newLibraryId);
		};

		client.on("library-changed", handleLibraryChange);
		return () => {
			client.off("library-changed", handleLibraryChange);
		};
	}, [client, wireMethod]);

	// Include library ID in key so switching libraries triggers refetch
	// useMemo to prevent array recreation on every render
	const queryKey = useMemo(
		() => [wireMethod, libraryId, input],
		[wireMethod, libraryId, JSON.stringify(input)],
	);

	// Use TanStack Query normally
	// When libraryId changes, queryKey changes, and TanStack Query automatically fetches new data
	const query = useQuery<O>({
		queryKey,
		queryFn: async () => {
			// Client.execute() automatically adds library_id to the request
			// as a sibling field to payload
			return await client.execute<I, O>(wireMethod, input);
		},
		enabled: enabled && !!libraryId,
	});

	// Listen for ResourceChanged events and update cache atomically
	useEffect(() => {
		// Helper: Check if event affects the pathScope (if specified)
		const eventAffectsPath = (metadata: any): boolean => {
			if (!pathScope) return true; // No path filter, accept all

			const affectedPaths = metadata?.affected_paths || [];
			if (affectedPaths.length === 0) return true; // Global resource, no paths

			// Check if any affected path matches our pathScope
			return affectedPaths.some((affectedPath: any) => {
				// Handle Physical paths with hierarchy
				if ("Physical" in pathScope && "Physical" in affectedPath) {
					// Handle both device_id (manual types) and device_slug (generated types)
					const scopeDevice =
						(pathScope.Physical as any).device_slug ||
						(pathScope.Physical as any).device_id;
					const scopePath = (pathScope.Physical as any).path;
					const fileDevice =
						(affectedPath.Physical as any).device_slug ||
						(affectedPath.Physical as any).device_id;
					const filePath = (affectedPath.Physical as any).path;

					// Must be same device AND file must be under scope directory
					return (
						scopeDevice === fileDevice &&
						filePath.startsWith(scopePath)
					);
				}

				// Handle Content ID paths
				if ("Content" in pathScope && "Content" in affectedPath) {
					const scope = pathScope as {
						Content: { content_id: string };
					};
					const affected = affectedPath as {
						Content: { content_id: string };
					};
					return (
						scope.Content.content_id === affected.Content.content_id
					);
				}

				// Handle Sidecar paths (match by content ID)
				if ("Content" in pathScope && "Sidecar" in affectedPath) {
					const scope = pathScope as {
						Content: { content_id: string };
					};
					const affected = affectedPath as {
						Sidecar: { content_id: string };
					};
					return (
						scope.Content.content_id === affected.Sidecar.content_id
					);
				}
				if ("Sidecar" in pathScope && "Content" in affectedPath) {
					const scope = pathScope as {
						Sidecar: { content_id: string };
					};
					const affected = affectedPath as {
						Content: { content_id: string };
					};
					return (
						scope.Sidecar.content_id === affected.Content.content_id
					);
				}

				// Handle Cloud paths
				if ("Cloud" in pathScope && "Cloud" in affectedPath) {
					const scope = pathScope as {
						Cloud: {
							service: string;
							identifier: string;
							path: string;
						};
					};
					const affected = affectedPath as {
						Cloud: {
							service: string;
							identifier: string;
							path: string;
						};
					};
					return (
						scope.Cloud.service === affected.Cloud.service &&
						scope.Cloud.identifier === affected.Cloud.identifier &&
						affected.Cloud.path.startsWith(scope.Cloud.path)
					);
				}

				// Fallback to exact match for unknown types
				return (
					JSON.stringify(affectedPath) === JSON.stringify(pathScope)
				);
			});
		};

		const handleEvent = (event: any) => {
			// Handle Refresh event - invalidate all queries
			if ("Refresh" in event) {
				console.log(
					"[useNormalizedCache] Refresh event received, invalidating all queries",
				);
				queryClient.invalidateQueries();
				return;
			}

			// Fast path: ignore job/indexing progress events immediately
			if ("JobProgress" in event || "IndexingProgress" in event) {
				return;
			}

			// Check if this is a ResourceChanged event for our resource type
			if ("ResourceChanged" in event) {
				const { resource_type, resource, metadata } =
					event.ResourceChanged;

				const noMergeFields = metadata?.no_merge_fields || [];

				// Log all events that match our resource type
				if (resource_type === resourceType)
					console.log(
						"targeted ResourceChanged event",
						resource_type,
						resourceType,
						event,
					);

				if (
					resource_type === resourceType &&
					eventAffectsPath(metadata)
				) {
					console.log("ResourceChanged event affects path", metadata);
					// Atomic update: merge this resource into the query data
					queryClient.setQueryData<O>(queryKey, (oldData) => {
						if (!oldData) {
							return oldData;
						}

						// Handle both array responses and wrapped responses
						// e.g., LocationsListOutput = { locations: LocationInfo[] }
						if (Array.isArray(oldData)) {
							// Direct array response
							const resourceId = resource.id;
							const existingIndex = oldData.findIndex(
								(item: any) => item.id === resourceId,
							);

							if (existingIndex >= 0) {
								const newData = [...oldData];
								newData[existingIndex] = deepMerge(
									oldData[existingIndex],
									resource,
									noMergeFields,
								);
								return newData as O;
							}

							// Append if no filter OR resource passes filter
							if (!resourceFilter || resourceFilter(resource)) {
								console.log(
									"[Cache] Appending new item to array",
								);
								return [...oldData, resource] as O;
							}

							console.log(
								"[Cache] Skipping - filtered out by resourceFilter",
							);

							return oldData;
						} else if (oldData && typeof oldData === "object") {
							// Wrapped response - look for array field
							// Try common wrapper field names
							const arrayField = Object.keys(oldData).find(
								(key) => Array.isArray((oldData as any)[key]),
							);

							if (arrayField) {
								const array = (oldData as any)[arrayField];
								const resourceId = resource.id;
								const existingIndex = array.findIndex(
									(item: any) => item.id === resourceId,
								);

								if (existingIndex >= 0) {
									const newArray = [...array];
									newArray[existingIndex] = deepMerge(
										array[existingIndex],
										resource,
										noMergeFields,
									);
									console.log(
										`[${resource_type}] Updated existing item in wrapped array`,
										{
											wireMethod,
											field: arrayField,
											id: resource.id,
										},
									);
									return {
										...oldData,
										[arrayField]: newArray,
									};
								}

								// Append if no filter OR resource passes filter
								if (
									!resourceFilter ||
									resourceFilter(resource)
								) {
									console.log(
										`[${resource_type}] Appended to wrapped array`,
										{
											wireMethod,
											field: arrayField,
											id: resource.id,
										},
									);
									return {
										...oldData,
										[arrayField]: [...array, resource],
									};
								}

								return oldData;
							}

							// Check for wrapped single-object field (e.g., { layout: SpaceLayout })
							for (const key of Object.keys(oldData)) {
								const wrappedValue = (oldData as any)[key];
								if (
									wrappedValue &&
									typeof wrappedValue === "object" &&
									!Array.isArray(wrappedValue) &&
									wrappedValue.id === resource.id
								) {
									console.log(
										`[${resource_type}] Updated wrapped object`,
										{
											wireMethod,
											field: key,
											id: resource.id,
										},
									);
									return {
										...oldData,
										[key]: deepMerge(
											wrappedValue,
											resource,
											noMergeFields,
										),
									} as O;
								}
							}

							// Handle single object response (e.g., files.by_id returns a single File)
							// Check if oldData is a single resource object
							if ((oldData as any).id === resource.id) {
								// This is the file we're displaying - merge the update
								// console.log('[Cache] Updating single resource:', {
								//   oldId: (oldData as any).id,
								//   newId: resource.id,
								//   name: resource.name,
								// });
								return deepMerge(
									oldData,
									resource,
									noMergeFields,
								) as O;
							}

							// Also check by content UUID for single object
							if (
								(oldData as any).content_identity?.uuid &&
								(oldData as any).content_identity.uuid ===
									resource.content_identity?.uuid
							) {
								// console.log('[Cache] Updating single resource by content UUID:', {
								//   contentId: resource.content_identity.uuid,
								//   name: resource.name,
								// });
								return deepMerge(
									oldData,
									resource,
									noMergeFields,
								) as O;
							}
						}

						return oldData;
					});
				}
			} else if ("ResourceChangedBatch" in event) {
				const { resource_type, resources, metadata } =
					event.ResourceChangedBatch;

				// Log all batch events that match our resource type
				if (resource_type === resourceType) {
					console.log(
						"targeted ResourceChangedBatch event",
						resource_type,
						resourceType,
						"passes path filter:",
						eventAffectsPath(metadata),
						metadata,
					);
				}

				if (
					resource_type === resourceType &&
					Array.isArray(resources) &&
					eventAffectsPath(metadata)
				) {
					// Filter to matching resourceId if specified (for single-resource queries)
					const filteredResources = resourceId
						? resources.filter((r: any) => r.id === resourceId)
						: resources;

					if (filteredResources.length === 0) {
						return; // No matching resources for this query
					}
					// Extract merge config from Identifiable metadata
					const noMergeFields = metadata?.no_merge_fields || [];
					const alternateIds = metadata?.alternate_ids || [];

					// Atomic update: merge filtered resources into the query data
					queryClient.setQueryData<O>(queryKey, (oldData) => {
						if (!oldData) return oldData;

						// Helper: check if resource matches by ID or alternate IDs
						const matches = (existing: any, incoming: any) => {
							if (existing.id === incoming.id) return true;
							// Check alternate IDs (e.g., content UUID for Files)
							return alternateIds.some(
								(altId: any) =>
									existing.id === altId ||
									existing.content_identity?.uuid === altId ||
									incoming.id === altId ||
									incoming.content_identity?.uuid === altId,
							);
						};

						// Create a map of filtered incoming resources
						const resourceMap = new Map(
							filteredResources.map((r: any) => [r.id, r]),
						);

						if (Array.isArray(oldData)) {
							// Direct array response
							const newData = [...oldData];
							const seenIds = new Set();

							// Update existing items with deep merge
							for (let i = 0; i < newData.length; i++) {
								const item: any = newData[i];
								if (resourceMap.has(item.id)) {
									const incomingResource = resourceMap.get(
										item.id,
									);
									newData[i] = deepMerge(
										item,
										incomingResource,
									);
									seenIds.add(item.id);
								}
							}

							// Append new items if no filter OR resource passes filter
							for (const resource of resources) {
								if (seenIds.has(resource.id)) {
									continue; // Already updated by ID
								}

								// Check if we should process this resource
								if (
									resourceFilter &&
									!resourceFilter(resource)
								) {
									continue; // Filtered out
								}

								// For Content-based paths with multiple entries, update by content UUID
								// (sidecar events can create multiple File resources for the same content)
								if (
									resource.sd_path?.Content &&
									resource.content_identity?.uuid
								) {
									const contentId =
										resource.content_identity.uuid;

									// Find existing item with same content
									const existingIndex = newData.findIndex(
										(item: any) =>
											item.content_identity?.uuid ===
											contentId,
									);

									if (existingIndex >= 0) {
										// Update existing item (merge sidecars, etc.)
										newData[existingIndex] = deepMerge(
											newData[existingIndex],
											resource,
											noMergeFields,
										);
										console.log(
											"[Cache] Updated existing file by content UUID:",
											{
												name: resource.name,
												contentId,
											},
										);
										continue;
									}
								}

								// New item - append it
								newData.push(resource);
							}

							return newData as O;
						} else if (oldData && typeof oldData === "object") {
							// Check if this is a single resource object vs wrapper
							// Single resource has: id field
							// Wrapper has: array field (files, locations, etc.) + pagination fields
							const isSingleResource = !!(oldData as any).id;

							// console.log("[Cache] Batch - response type check:", {
							//   isSingleResource,
							//   hasId: !!(oldData as any).id,
							//   hasSdPath: !!(oldData as any).sd_path,
							//   firstKey: Object.keys(oldData)[0],
							// });

							if (isSingleResource) {
								// For File resources with sd_path, validate path matches (prevent cross-path pollution)
								const oldPath = (oldData as any).sd_path;

								if (oldPath) {
									// This is a File with a path - filter to matching path only
									const filteredByPath =
										filteredResources.filter(
											(resource: any) => {
												if (!resource.sd_path)
													return false;

												// Deep compare sd_path objects
												return (
													JSON.stringify(oldPath) ===
													JSON.stringify(
														resource.sd_path,
													)
												);
											},
										);

									if (filteredByPath.length === 0) {
										return oldData; // No matching paths, don't update
									}

									// Update to only process path-matching resources
									filteredResources.length = 0;
									filteredResources.push(...filteredByPath);
									resourceMap.clear();
									filteredByPath.forEach((r) =>
										resourceMap.set(r.id, r),
									);
								}

								// For non-File resources (SpaceLayout, etc), no path filtering needed
								// They're already filtered by resourceId above

								// Single object response - check each incoming resource
								for (const resource of filteredResources) {
									// Match by ID
									if ((oldData as any).id === resource.id) {
										console.log(
											"[Cache] ✓ Updating single object by ID:",
											{
												name: resource.name,
												id: resource.id,
											},
										);
										return deepMerge(
											oldData,
											resource,
											noMergeFields,
										) as O;
									}

									// Match by content UUID
									if (
										(oldData as any).content_identity
											?.uuid &&
										(oldData as any).content_identity
											.uuid ===
											resource.content_identity?.uuid
									) {
										console.log(
											"[Cache] ✓ Updating single object by content UUID:",
											{
												name: resource.name,
												contentId:
													resource.content_identity
														.uuid,
											},
										);
										return deepMerge(
											oldData,
											resource,
											noMergeFields,
										) as O;
									}
								}

								console.log(
									"[Cache] ✗ No match found for single object",
								);
								// No match - return unchanged
								return oldData;
							}

							// Wrapped response with array field
							const arrayField = Object.keys(oldData).find(
								(key) => Array.isArray((oldData as any)[key]),
							);

							if (arrayField) {
								const array = [...(oldData as any)[arrayField]];
								const seenIds = new Set();

								// Update existing items with deep merge
								for (let i = 0; i < array.length; i++) {
									const item: any = array[i];
									if (resourceMap.has(item.id)) {
										const incomingResource =
											resourceMap.get(item.id);
										array[i] = deepMerge(
											item,
											incomingResource,
										);
										seenIds.add(item.id);
									}
								}

								// Append new items if no filter OR resource passes filter
								for (const resource of resources) {
									if (seenIds.has(resource.id)) {
										continue; // Already updated by ID
									}

									// Check if we should process this resource
									if (
										resourceFilter &&
										!resourceFilter(resource)
									) {
										continue; // Filtered out
									}

									// For Content-based paths, update existing item by content UUID
									if (
										resource.sd_path?.Content &&
										resource.content_identity?.uuid
									) {
										const contentId =
											resource.content_identity.uuid;
										const existingIndex = array.findIndex(
											(item: any) =>
												item.content_identity?.uuid ===
												contentId,
										);

										if (existingIndex >= 0) {
											// Update existing item
											array[existingIndex] = deepMerge(
												array[existingIndex],
												resource,
												noMergeFields,
											);
											console.log(
												"[Cache] Updated existing file by content UUID:",
												{
													name: resource.name,
													contentId,
												},
											);
											continue;
										}
									}

									// New item - append
									array.push(resource);
								}

								return { ...oldData, [arrayField]: array };
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
							return oldData.filter(
								(item: any) => item.id !== resource_id,
							) as O;
						} else if (oldData && typeof oldData === "object") {
							const arrayField = Object.keys(oldData).find(
								(key) => Array.isArray((oldData as any)[key]),
							);

							if (arrayField) {
								const array = (oldData as any)[arrayField];
								return {
									...oldData,
									[arrayField]: array.filter(
										(item: any) => item.id !== resource_id,
									),
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
	}, [resourceType, queryKey, queryClient, pathScope]);

	return query;
}
