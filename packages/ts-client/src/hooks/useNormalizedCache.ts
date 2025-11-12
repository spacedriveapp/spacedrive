import { useEffect } from "react";
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
function deepMerge(existing: any, incoming: any, noMergeFields: string[] = []): any {
  // If incoming is null/undefined, keep existing
  if (incoming === null || incoming === undefined) {
    return existing !== null && existing !== undefined ? existing : incoming;
  }

  // If types don't match or not objects, incoming wins
  if (typeof existing !== 'object' || typeof incoming !== 'object' ||
      Array.isArray(existing) || Array.isArray(incoming)) {
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
    } else if (typeof existing[key] === 'object' && typeof incoming[key] === 'object' &&
               !Array.isArray(existing[key]) && !Array.isArray(incoming[key])) {
      // Both are objects - recurse
      merged[key] = deepMerge(existing[key], incoming[key], noMergeFields);
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
  alternateIds: string[] = []
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
  /** Whether this is a global list query that should accept new items (default: false) */
  isGlobalList?: boolean;
  /** Optional filter function to check if a resource belongs in this query */
  resourceFilter?: (resource: any) => boolean;
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
  isGlobalList = false,
  resourceFilter,
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
      // as a sibling field to payload
      return await client.execute<I, O>(wireMethod, input);
    },
    enabled: enabled && !!libraryId,
  });

  // Listen for ResourceChanged events and update cache atomically
  useEffect(() => {
    const handleEvent = (event: any) => {
      // Fast path: ignore job/indexing progress events immediately
      if ("JobProgress" in event || "IndexingProgress" in event) {
        return;
      }

      // Check if this is a ResourceChanged event for our resource type
      if ("ResourceChanged" in event) {
        const { resource_type, resource, metadata } = event.ResourceChanged;

          const noMergeFields = metadata?.no_merge_fields || [];

        console.log('[ResourceEvent] ResourceChanged:', {
          resourceType: resource_type,
          ourType: resourceType,
          resourceName: resource?.name,
          resourceId: resource?.id,
        });

        if (resource_type === resourceType) {
          // Atomic update: merge this resource into the query data
          queryClient.setQueryData<O>(queryKey, (oldData) => {
            if (!oldData) return oldData;

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
                newData[existingIndex] = deepMerge(oldData[existingIndex], resource, noMergeFields);
                return newData as O;
              }

              // Append if this is a global list OR resource passes filter
              if (isGlobalList || (resourceFilter && resourceFilter(resource))) {
                return [...oldData, resource] as O;
              }

              return oldData;
            } else if (oldData && typeof oldData === "object") {
              // Wrapped response - look for array field
              // Try common wrapper field names
              const arrayField = Object.keys(oldData).find((key) =>
                Array.isArray((oldData as any)[key]),
              );

              if (arrayField) {
                const array = (oldData as any)[arrayField];
                const resourceId = resource.id;
                const existingIndex = array.findIndex(
                  (item: any) => item.id === resourceId,
                );

                if (existingIndex >= 0) {
                  const newArray = [...array];
                  newArray[existingIndex] = deepMerge(array[existingIndex], resource, noMergeFields);
                  return { ...oldData, [arrayField]: newArray };
                }

                // Append if this is a global list OR resource passes filter
                if (isGlobalList || (resourceFilter && resourceFilter(resource))) {
                  return { ...oldData, [arrayField]: [...array, resource] };
                }

                return oldData;
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
                return deepMerge(oldData, resource, noMergeFields) as O;
              }

              // Also check by content UUID for single object
              if (
                (oldData as any).content_identity?.uuid &&
                (oldData as any).content_identity.uuid === resource.content_identity?.uuid
              ) {
                // console.log('[Cache] Updating single resource by content UUID:', {
                //   contentId: resource.content_identity.uuid,
                //   name: resource.name,
                // });
                return deepMerge(oldData, resource, noMergeFields) as O;
              }
            }

            return oldData;
          });
        }
      } else if ("ResourceChangedBatch" in event) {
        const { resource_type, resources, metadata } = event.ResourceChangedBatch;

        // console.log('[ResourceEvent] ResourceChangedBatch:', {
        //   resourceType: resource_type,
        //   ourType: resourceType,
        //   count: resources?.length,
        //   firstResource: resources?.[0]?.name,
        // });

        if (resource_type === resourceType && Array.isArray(resources)) {
          // Extract merge config from Identifiable metadata
          const noMergeFields = metadata?.no_merge_fields || [];
          const alternateIds = metadata?.alternate_ids || [];

          // Atomic update: merge all resources into the query data
          queryClient.setQueryData<O>(queryKey, (oldData) => {
            if (!oldData) return oldData;

            // Helper: check if resource matches by ID or alternate IDs
            const matches = (existing: any, incoming: any) => {
              if (existing.id === incoming.id) return true;
              // Check alternate IDs (e.g., content UUID for Files)
              return alternateIds.some(altId =>
                existing.id === altId ||
                existing.content_identity?.uuid === altId ||
                incoming.id === altId ||
                incoming.content_identity?.uuid === altId
              );
            };

            // Create a map of incoming resources
            const resourceMap = new Map(resources.map((r: any) => [r.id, r]));

            if (Array.isArray(oldData)) {
              // Direct array response
              const newData = [...oldData];
              const seenIds = new Set();

              // Update existing items with deep merge
              for (let i = 0; i < newData.length; i++) {
                const item: any = newData[i];
                if (resourceMap.has(item.id)) {
                  const incomingResource = resourceMap.get(item.id);
                  newData[i] = deepMerge(item, incomingResource);
                  seenIds.add(item.id);
                }
              }

              // Append new items if:
              // - This is a global list query, OR
              // - The resource passes the filter (belongs in this query scope)
              if (isGlobalList) {
                for (const resource of resources) {
                  if (!seenIds.has(resource.id)) {
                    newData.push(resource);
                  }
                }
              } else if (resourceFilter) {
                for (const resource of resources) {
                  if (seenIds.has(resource.id)) {
                    continue; // Already updated by ID
                  }

                  // Check if we should process this resource
                  const shouldAppend = resourceFilter(resource);
                  if (!shouldAppend) {
                    continue;
                  }

                  // For Content-based paths with multiple entries, update by content UUID
                  // (sidecar events can create multiple File resources for the same content)
                  if (resource.sd_path?.Content && resource.content_identity?.uuid) {
                    const contentId = resource.content_identity.uuid;

                    // Find existing item with same content
                    const existingIndex = newData.findIndex(
                      (item: any) => item.content_identity?.uuid === contentId
                    );

                    if (existingIndex >= 0) {
                      // Update existing item (merge sidecars, etc.)
                      newData[existingIndex] = deepMerge(newData[existingIndex], resource, noMergeFields);
                      console.log('[Cache] Updated existing file by content UUID:', {
                        name: resource.name,
                        contentId,
                      });
                      continue;
                    }
                  }

                  // New item - append it
                  newData.push(resource);
                }
              }

              return newData as O;
            } else if (oldData && typeof oldData === "object") {
              // Check if this is a single resource object (File) vs wrapper ({files: [...]})
              // Single resource has: id, name, sd_path
              // Wrapper has: files (array), has_more, total_count
              const isSingleResource = !!(oldData as any).id && !!(oldData as any).sd_path;

              console.log('[Cache] Batch - response type check:', {
                isSingleResource,
                hasId: !!(oldData as any).id,
                hasSdPath: !!(oldData as any).sd_path,
                firstKey: Object.keys(oldData)[0],
              });

              if (isSingleResource) {
                // Single object response - check each incoming resource
                console.log('[Cache] Single object mode - checking resources:', {
                  oldDataId: (oldData as any).id,
                  oldDataContentId: (oldData as any).content_identity?.uuid,
                  incomingCount: resources.length,
                  incomingIds: resources.map((r: any) => r.id),
                  incomingContentIds: resources.map((r: any) => r.content_identity?.uuid),
                });

                for (const resource of resources) {
                  // Match by ID
                  if ((oldData as any).id === resource.id) {
                    console.log('[Cache] ✓ Updating single object by ID:', {
                      name: resource.name,
                      id: resource.id,
                    });
                    return deepMerge(oldData, resource, noMergeFields) as O;
                  }

                  // Match by content UUID
                  if (
                    (oldData as any).content_identity?.uuid &&
                    (oldData as any).content_identity.uuid === resource.content_identity?.uuid
                  ) {
                    console.log('[Cache] ✓ Updating single object by content UUID:', {
                      name: resource.name,
                      contentId: resource.content_identity.uuid,
                    });
                    return deepMerge(oldData, resource, noMergeFields) as O;
                  }
                }

                console.log('[Cache] ✗ No match found for single object');
                // No match - return unchanged
                return oldData;
              }

              // Wrapped response with array field
              const arrayField = Object.keys(oldData).find((key) =>
                Array.isArray((oldData as any)[key]),
              );

              if (arrayField) {
                const array = [...(oldData as any)[arrayField]];
                const seenIds = new Set();

                // Update existing items with deep merge
                for (let i = 0; i < array.length; i++) {
                  const item: any = array[i];
                  if (resourceMap.has(item.id)) {
                    const incomingResource = resourceMap.get(item.id);
                    array[i] = deepMerge(item, incomingResource);
                    seenIds.add(item.id);
                  }
                }

                // Append new items if:
                // - This is a global list query, OR
                // - The resource passes the filter (belongs in this query scope)
                if (isGlobalList) {
                  for (const resource of resources) {
                    if (!seenIds.has(resource.id)) {
                      array.push(resource);
                    }
                  }
                } else if (resourceFilter) {
                  for (const resource of resources) {
                    if (seenIds.has(resource.id)) {
                      continue; // Already updated by ID
                    }

                    const shouldAppend = resourceFilter(resource);
                    if (!shouldAppend) {
                      continue;
                    }

                    // For Content-based paths, update existing item by content UUID
                    if (resource.sd_path?.Content && resource.content_identity?.uuid) {
                      const contentId = resource.content_identity.uuid;
                      const existingIndex = array.findIndex(
                        (item: any) => item.content_identity?.uuid === contentId
                      );

                      if (existingIndex >= 0) {
                        // Update existing item
                        array[existingIndex] = deepMerge(array[existingIndex], resource, noMergeFields);
                        console.log('[Cache] Updated existing file by content UUID:', {
                          name: resource.name,
                          contentId,
                        });
                        continue;
                      }
                    }

                    // New item - append
                    array.push(resource);
                  }
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
              const arrayField = Object.keys(oldData).find((key) =>
                Array.isArray((oldData as any)[key]),
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
  }, [resourceType, queryKey, queryClient]);

  return query;
}
