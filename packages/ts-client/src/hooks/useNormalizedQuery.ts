/**
 * useNormalizedQuery - Normalized cache with real-time updates
 *
 * A typesafe TanStack Query wrapper providing instant cache updates
 * via filtered WebSocket subscriptions. The counterpart to the Identifiable
 * trait in the Rust core, processing ResourceEvents to update the cache.
 * - Runtime type safety with Valibot
 * - Deep merging with ts-deepmerge
 * - Stable callbacks with React 19 useEvent
 * - Rrror handling with tiny-invariant
 *
 * ## Example
 *
 * ```tsx
 * const { data: files } = useNormalizedQuery({
 *   wireMethod: 'query:files.directory_listing',
 *   input: { path: currentPath },
 *   resourceType: 'file',
 *   pathScope: currentPath,
 *   includeDescendants: false, // Exact mode - only direct children
 * });
 * ```
 */

import { useEffect, useMemo, useState, useRef } from "react";
import { useQuery, useQueryClient, QueryClient } from "@tanstack/react-query";
import { useSpacedriveClient } from "./useClient";
import type { Event } from "../generated/types";
import invariant from "tiny-invariant";
import * as v from "valibot";
import type { Simplify } from "type-fest";

// Types

export type UseNormalizedQueryOptions<I> = Simplify<{
	/** Wire method to call (e.g., "query:files.directory_listing") */
	wireMethod: string;
	/** Input for the query */
	input: I;
	/** Resource type for event filtering (e.g., "file", "location") */
	resourceType: string;
	/** Whether query is enabled (default: true) */
	enabled?: boolean;
	/** Optional path scope for server-side filtering */
	pathScope?: any; // SdPath type
	/** Whether to include descendants (recursive) or only direct children (exact) */
	includeDescendants?: boolean;
	/** Resource ID for single-resource queries */
	resourceId?: string;
	/** Enable debug logging for this query instance */
	debug?: boolean;
}>;

// Runtime Validation Schemas (Valibot)

const ResourceChangedSchema = v.object({
	ResourceChanged: v.object({
		resource_type: v.string(),
		resource: v.any(),
		metadata: v.nullish(
			v.object({
				no_merge_fields: v.optional(v.array(v.string())),
				affected_paths: v.optional(v.array(v.any())),
				alternate_ids: v.optional(v.array(v.any())),
			}),
		),
	}),
});

const ResourceChangedBatchSchema = v.object({
	ResourceChangedBatch: v.object({
		resource_type: v.string(),
		resources: v.array(v.any()),
		metadata: v.nullish(
			v.object({
				no_merge_fields: v.optional(v.array(v.string())),
				affected_paths: v.optional(v.array(v.any())),
				alternate_ids: v.optional(v.array(v.any())),
			}),
		),
	}),
});

const ResourceDeletedSchema = v.object({
	ResourceDeleted: v.object({
		resource_type: v.string(),
		resource_id: v.string(),
	}),
});

// Main Hook

/**
 * useNormalizedQuery - Main hook
 */
export function useNormalizedQuery<I, O>(
	options: UseNormalizedQueryOptions<I>,
) {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();
	const [libraryId, setLibraryId] = useState<string | null>(
		client.getCurrentLibraryId(),
	);

	// Listen for library changes
	useEffect(() => {
		const handleLibraryChange = (newLibraryId: string) => {
			setLibraryId(newLibraryId);
		};

		client.on("library-changed", handleLibraryChange);
		return () => {
			client.off("library-changed", handleLibraryChange);
		};
	}, [client]);

	// Query key
	const queryKey = useMemo(
		() => [options.wireMethod, libraryId, options.input],
		[options.wireMethod, libraryId, JSON.stringify(options.input)],
	);

	// Standard TanStack Query
	const query = useQuery<O>({
		queryKey,
		queryFn: async () => {
			invariant(libraryId, "Library ID must be set before querying");
			return await client.execute<I, O>(
				options.wireMethod,
				options.input,
			);
		},
		enabled: (options.enabled ?? true) && !!libraryId,
	});

	// Refs for stable access to latest values without triggering re-subscription
	const optionsRef = useRef(options);
	const queryKeyRef = useRef(queryKey);

	// Update refs on every render
	useEffect(() => {
		optionsRef.current = options;
		queryKeyRef.current = queryKey;
	});

	// Serialize pathScope for deep comparison in dependency array
	// This ensures subscription re-runs when path changes, even if object reference stays same
	const pathScopeSerialized = useMemo(
		() => JSON.stringify(options.pathScope),
		[options.pathScope],
	);

	// Event subscription
	// Only re-subscribe when filter criteria change
	// Using refs for event handler to avoid re-subscription on every render
	useEffect(() => {
		if (!libraryId) return;

		// Skip subscription for file queries without pathScope (prevent overly broad subscriptions)
		// File resources are too numerous - global subscriptions cause massive event spam
		// Single-file queries (FileInspector) will use stale-while-revalidate instead
		if (options.resourceType === "file" && !options.pathScope) {
			return;
		}

		let unsubscribe: (() => void) | undefined;
		let isCancelled = false;

		// Capture current pathScope in closure to prevent stale events from updating wrong query
		const capturedPathScope = options.pathScope;
		const capturedQueryKey = queryKey;

		const handleEvent = (event: Event) => {
			// Guard: only process events if pathScope hasn't changed since subscription
			if (
				JSON.stringify(optionsRef.current.pathScope) !==
				JSON.stringify(capturedPathScope)
			) {
				return;
			}

			handleResourceEvent(
				event,
				optionsRef.current,
				capturedQueryKey, // Use captured queryKey, not ref
				queryClient,
				optionsRef.current.debug, // Pass debug flag
			);
		};

		client
			.subscribeFiltered(
				{
					resource_type: options.resourceType,
					path_scope: options.pathScope,
					library_id: libraryId,
					include_descendants: options.includeDescendants ?? false,
				},
				handleEvent,
			)
			.then((unsub) => {
				if (isCancelled) {
					unsub();
				} else {
					unsubscribe = unsub;
				}
			});

		return () => {
			isCancelled = true;
			unsubscribe?.();
		};
	}, [
		client,
		queryClient,
		options.resourceType,
		options.resourceId,
		pathScopeSerialized, // Use serialized version for deep comparison
		options.includeDescendants,
		libraryId,
		// options and queryKey accessed via refs - don't need to be in deps
	]);

	return query;
}

// Event Handling

/**
 * Event handler dispatcher with runtime validation
 *
 * Routes validated events to appropriate update functions.
 * Exported for testing.
 */
export function handleResourceEvent(
	event: Event,
	options: UseNormalizedQueryOptions<any>,
	queryKey: any[],
	queryClient: QueryClient,
	debug?: boolean,
) {
	// Skip string events (like "CoreStarted", "CoreShutdown")
	if (typeof event === "string") {
		return;
	}

	// Refresh event - invalidate all queries
	if ("Refresh" in event) {
		if (debug) {
			console.log(
				`[useNormalizedQuery] ${options.wireMethod} processing Refresh`,
				event,
			);
		}
		queryClient.invalidateQueries();
		return;
	}

	// Single resource changed - validate and process
	if ("ResourceChanged" in event) {
		const result = v.safeParse(ResourceChangedSchema, event);
		if (!result.success) {
			return;
		}

		const { resource_type, resource, metadata } =
			result.output.ResourceChanged;
		if (resource_type === options.resourceType) {
			if (debug) {
				console.log(
					`[useNormalizedQuery] ${options.wireMethod} processing ResourceChanged`,
					event,
				);
			}
			updateSingleResource(
				resource,
				metadata,
				queryKey,
				queryClient,
				options,
			);
		}
	}

	// Batch resource changed - validate and process
	else if ("ResourceChangedBatch" in event) {
		const result = v.safeParse(ResourceChangedBatchSchema, event);
		if (!result.success) {
			return;
		}

		const { resource_type, resources, metadata } =
			result.output.ResourceChangedBatch;

		if (
			resource_type === options.resourceType &&
			Array.isArray(resources)
		) {
			if (debug) {
				console.log(
					`[useNormalizedQuery] ${options.wireMethod} processing ResourceChangedBatch`,
					event,
				);
			}
			updateBatchResources(
				resources,
				metadata,
				options,
				queryKey,
				queryClient,
			);
		}
	}

	// Resource deleted - validate and process
	else if ("ResourceDeleted" in event) {
		const result = v.safeParse(ResourceDeletedSchema, event);
		if (!result.success) {
			return;
		}

		const { resource_type, resource_id } = result.output.ResourceDeleted;
		if (resource_type === options.resourceType) {
			if (debug) {
				console.log(
					`[useNormalizedQuery] ${options.wireMethod} processing ResourceDeleted`,
					event,
				);
			}
			deleteResource(resource_id, queryKey, queryClient);
		}
	}
}

// Batch Filtering

/**
 * Filter batch resources by pathScope for exact mode
 *
 * ## Why This Exists
 *
 * Server-side filtering reduces events by 90%+, but can't split atomic batches.
 * If a batch has 100 files and 1 belongs to our scope, the entire batch is sent.
 * This client-side filter ensures only relevant resources are cached.
 *
 * ## The Critical Bug This Prevents
 *
 * Scenario: Viewing /Desktop, indexing creates batch with:
 * - /Desktop/file1.txt (direct child)
 * - /Desktop/Subfolder/file2.txt (grandchild)
 *
 * Without filtering: Both files appear in /Desktop view
 * With filtering: Only file1.txt appears
 *
 * @param resources - Resources from batch event
 * @param options - Query options
 * @returns Filtered resources for this query scope
 *
 * Exported for testing
 */
export function filterBatchResources(
	resources: any[],
	options: UseNormalizedQueryOptions<any>,
): any[] {
	let filtered = resources;

	// Filter by resourceId (single-resource queries like file inspector)
	if (options.resourceId) {
		filtered = filtered.filter((r: any) => r.id === options.resourceId);
	}

	// Filter by pathScope for file resources in exact mode
	if (
		options.pathScope &&
		options.resourceType === "file" &&
		!options.includeDescendants
	) {
		const beforeCount = filtered.length;
		filtered = filtered.filter((resource: any) => {
			// Get the scope path (must be Physical)
			const scopeStr = (options.pathScope as any).Physical?.path;
			if (!scopeStr) {
				return false; // No Physical scope path
			}

			// Normalize scope: remove trailing slashes for consistent comparison
			const normalizedScope = String(scopeStr).replace(/\/+$/, "");

			// Try to find a Physical path - check alternate_paths first, then sd_path
			const alternatePaths = resource.alternate_paths || [];
			const physicalFromAlternate = alternatePaths.find(
				(p: any) => p.Physical,
			);
			const physicalFromSdPath = resource.sd_path?.Physical;

			const physicalPath =
				physicalFromAlternate?.Physical || physicalFromSdPath;

			if (!physicalPath?.path) {
				return false; // No physical path found
			}

			const pathStr = String(physicalPath.path);

			// Extract parent directory from file path
			const lastSlash = pathStr.lastIndexOf("/");
			if (lastSlash === -1) {
				return false; // File path has no parent directory
			}

			const parentDir = pathStr.substring(0, lastSlash);

			// Only match if parent equals scope (normalized)
			return parentDir === normalizedScope;
		});
	}

	return filtered;
}

// Cache Update Functions

/**
 * Update a single resource using type-safe deep merge
 *
 * Exported for testing
 */
export function updateSingleResource<O>(
	resource: any,
	metadata: any,
	queryKey: any[],
	queryClient: QueryClient,
	options?: UseNormalizedQueryOptions<any>,
) {
	const noMergeFields = metadata?.no_merge_fields || [];

	// Apply client-side filtering if options provided (same as batch)
	let resourcesToUpdate = [resource];
	if (options) {
		resourcesToUpdate = filterBatchResources(resourcesToUpdate, options);
		if (resourcesToUpdate.length === 0) {
			// Resource was filtered out - may have moved out of scope, remove from cache
			if (resource.id) {
				deleteResource(resource.id, queryKey, queryClient);
			}
			return;
		}
	}

	queryClient.setQueryData<O>(queryKey, (oldData: any) => {
		if (!oldData) return oldData;

		// Handle array responses
		if (Array.isArray(oldData)) {
			return updateArrayCache(
				oldData,
				resourcesToUpdate,
				noMergeFields,
			) as O;
		}

		// Handle wrapped responses { files: [...] }
		if (oldData && typeof oldData === "object") {
			return updateWrappedCache(
				oldData,
				resourcesToUpdate,
				noMergeFields,
			) as O;
		}

		return oldData;
	});
}

/**
 * Update batch resources with filtering and deep merge
 *
 * Exported for testing
 */
export function updateBatchResources<O>(
	resources: any[],
	metadata: any,
	options: UseNormalizedQueryOptions<any>,
	queryKey: any[],
	queryClient: QueryClient,
) {
	const noMergeFields = metadata?.no_merge_fields || [];

	// Apply client-side filtering (safety fallback)
	const filteredResources = filterBatchResources(resources, options);

	// If all resources were filtered out, they may have moved OUT of scope
	// Remove them from cache if they exist (handles file moves out of current view)
	if (filteredResources.length === 0) {
		for (const resource of resources) {
			if (resource.id) {
				deleteResource(resource.id, queryKey, queryClient);
			}
		}
		return;
	}

	queryClient.setQueryData<O>(queryKey, (oldData: any) => {
		if (!oldData) return oldData;

		// Handle array responses
		if (Array.isArray(oldData)) {
			return updateArrayCache(
				oldData,
				filteredResources,
				noMergeFields,
			) as O;
		}

		// Handle wrapped responses { files: [...] }
		if (oldData && typeof oldData === "object") {
			return updateWrappedCache(
				oldData,
				filteredResources,
				noMergeFields,
			) as O;
		}

		return oldData;
	});
}

/**
 * Delete a resource from cache
 *
 * Exported for testing
 */
export function deleteResource<O>(
	resourceId: string,
	queryKey: any[],
	queryClient: QueryClient,
) {
	queryClient.setQueryData<O>(queryKey, (oldData: any) => {
		if (!oldData) return oldData;

		if (Array.isArray(oldData)) {
			return oldData.filter((item: any) => item.id !== resourceId) as O;
		}

		if (oldData && typeof oldData === "object") {
			const arrayField = Object.keys(oldData).find((key) =>
				Array.isArray((oldData as any)[key]),
			);

			if (arrayField) {
				return {
					...oldData,
					[arrayField]: (oldData as any)[arrayField].filter(
						(item: any) => item.id !== resourceId,
					),
				};
			}
		}

		return oldData;
	});
}

// Cache Update Helpers

/**
 * Update array cache (direct array response)
 */
function updateArrayCache(
	oldData: any[],
	newResources: any[],
	noMergeFields: string[],
): any[] {
	const newData = [...oldData];
	const seenIds = new Set();

	// Update existing items by ID
	for (let i = 0; i < newData.length; i++) {
		const item: any = newData[i];
		const match = newResources.find((r: any) => r.id === item.id);
		if (match) {
			newData[i] = safeMerge(item, match, noMergeFields);
			seenIds.add(match.id);
		}
	}

	// Handle Content entries that represent the same file as an existing Physical entry
	// When content identification happens, a new Content entry is created with a different ID
	// We need to merge it into the existing Physical entry by matching paths
	for (const resource of newResources) {
		if (!seenIds.has(resource.id) && resource.sd_path?.Content) {
			// Try to find existing Physical entry by matching alternate_paths
			const physicalPath = resource.alternate_paths?.find(
				(p: any) => p.Physical,
			)?.Physical?.path;
			if (physicalPath) {
				const existingIndex = newData.findIndex((item: any) => {
					const itemPath =
						item.sd_path?.Physical?.path ||
						item.alternate_paths?.find((p: any) => p.Physical)
							?.Physical?.path;
					return itemPath === physicalPath;
				});

				if (existingIndex !== -1) {
					// Merge Content entry into existing Physical entry
					newData[existingIndex] = safeMerge(
						newData[existingIndex],
						resource,
						noMergeFields,
					);
					seenIds.add(resource.id);
				}
			}
		}
	}

	// Append new items (excluding Content paths that didn't match an existing entry)
	for (const resource of newResources) {
		if (!seenIds.has(resource.id)) {
			// For Content paths: only add if they don't belong to an existing Physical entry
			// Content paths without matching Physical entries are either:
			// 1. Files moved into this directory (have alternate_paths but no match) → ADD
			// 2. Metadata updates for files elsewhere (no relevant alternate_paths) → SKIP
			if (resource.sd_path?.Content) {
				// Skip if no alternate_paths (pure metadata update)
				if (
					!resource.alternate_paths ||
					resource.alternate_paths.length === 0
				) {
					continue;
				}
				// Otherwise, this is a real file that belongs here - add it
			}
			newData.push(resource);
		}
	}

	return newData;
}

/**
 * Update wrapped cache ({ files: [...], locations: [...], etc. })
 */
function updateWrappedCache(
	oldData: any,
	newResources: any[],
	noMergeFields: string[],
): any {
	// First check: if oldData has an id that matches incoming, merge directly
	// This handles single object responses like files.by_id
	const match = newResources.find((r: any) => r.id === oldData.id);
	if (match) {
		return safeMerge(oldData, match, noMergeFields);
	}

	// Second check: wrapped responses like { files: [...] }
	const arrayField = Object.keys(oldData).find((key) =>
		Array.isArray(oldData[key]),
	);

	if (arrayField) {
		const array = [...oldData[arrayField]];
		const seenIds = new Set();

		// Update existing by ID
		for (let i = 0; i < array.length; i++) {
			const item: any = array[i];
			const match = newResources.find((r: any) => r.id === item.id);
			if (match) {
				array[i] = safeMerge(item, match, noMergeFields);
				seenIds.add(match.id);
			}
		}

		// Handle Content entries that represent the same file as an existing Physical entry
		for (const resource of newResources) {
			if (!seenIds.has(resource.id) && resource.sd_path?.Content) {
				// Try to find existing Physical entry by matching alternate_paths
				const physicalPath = resource.alternate_paths?.find(
					(p: any) => p.Physical,
				)?.Physical?.path;
				if (physicalPath) {
					const existingIndex = array.findIndex((item: any) => {
						const itemPath =
							item.sd_path?.Physical?.path ||
							item.alternate_paths?.find((p: any) => p.Physical)
								?.Physical?.path;
						return itemPath === physicalPath;
					});

					if (existingIndex !== -1) {
						// Merge Content entry into existing Physical entry
						array[existingIndex] = safeMerge(
							array[existingIndex],
							resource,
							noMergeFields,
						);
						seenIds.add(resource.id);
					}
				}
			}
		}

		// Append new items (excluding Content paths that didn't match an existing entry)
		for (const resource of newResources) {
			if (!seenIds.has(resource.id)) {
				// For Content paths: only add if they don't belong to an existing Physical entry
				// Content paths without matching Physical entries are either:
				// 1. Files moved into this directory (have alternate_paths but no match) → ADD
				// 2. Metadata updates for files elsewhere (no relevant alternate_paths) → SKIP
				if (resource.sd_path?.Content) {
					// Skip if no alternate_paths (pure metadata update)
					if (
						!resource.alternate_paths ||
						resource.alternate_paths.length === 0
					) {
						continue;
					}
					// Otherwise, this is a real file that belongs here - add it
				}

				// Check if resource already exists in the array (by ID)
				const alreadyExists = array.some(
					(item: any) => item.id === resource.id,
				);

				if (alreadyExists) {
					continue;
				}

				// New resource - append it
				array.push(resource);
			}
		}

		return { ...oldData, [arrayField]: array };
	}

	return oldData;
}

/**
 * Safe deep merge for resource updates
 *
 * Arrays are REPLACED (not concatenated) because:
 * - sidecars: Server sends complete list, duplicating would corrupt data
 * - alternate_paths: Same - server is authoritative
 * - tags: Same pattern
 *
 * Only nested objects are deep merged (like content_identity).
 *
 * Exported for testing
 */
export function safeMerge(
	existing: any,
	incoming: any,
	noMergeFields: string[] = [],
): any {
	// Handle null/undefined
	if (incoming === null || incoming === undefined) {
		return existing !== null && existing !== undefined
			? existing
			: incoming;
	}

	// Shallow merge with incoming winning, but deep merge nested objects
	const result: any = { ...existing };

	for (const key of Object.keys(incoming)) {
		const incomingVal = incoming[key];
		const existingVal = existing[key];

		// noMergeFields: incoming always wins
		if (noMergeFields.includes(key)) {
			result[key] = incomingVal;
		}
		// Arrays: replace entirely (don't concatenate)
		else if (Array.isArray(incomingVal)) {
			result[key] = incomingVal;
		}
		// Nested objects: deep merge recursively
		else if (
			incomingVal !== null &&
			typeof incomingVal === "object" &&
			existingVal !== null &&
			typeof existingVal === "object" &&
			!Array.isArray(existingVal)
		) {
			result[key] = safeMerge(existingVal, incomingVal, noMergeFields);
		}
		// Primitives: incoming wins
		else {
			result[key] = incomingVal;
		}
	}

	return result;
}
