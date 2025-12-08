/**
 * useLocationChangeInvalidation - Invalidates directory listing queries when location index_mode changes
 *
 * When a user enables indexing for a location (index_mode changes from "none" to something else),
 * we need to refetch directory listings because:
 * - Before: Data came from ephemeral in-memory index
 * - After: Data comes from persistent database
 *
 * This hook subscribes to location events and invalidates affected queries.
 */

import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useSpacedriveClient } from "@sd/ts-client/hooks";
import type { Event, LocationInfo } from "@sd/ts-client";

export function useLocationChangeInvalidation() {
	const client = useSpacedriveClient();
	const queryClient = useQueryClient();
	const libraryId = client.getCurrentLibraryId();

	// Track previous index_mode for each location to detect changes
	const prevIndexModes = useRef<Map<string, string>>(new Map());

	useEffect(() => {
		if (!libraryId) return;

		let unsubscribe: (() => void) | undefined;
		let isCancelled = false;

		const handleEvent = (event: Event) => {
			// Only handle ResourceChanged events for locations
			if (typeof event === "string" || !("ResourceChanged" in event)) {
				return;
			}

			const { resource_type, resource } = event.ResourceChanged;
			if (resource_type !== "location") {
				return;
			}

			const location = resource as LocationInfo;
			const locationId = location.id;
			const newIndexMode = location.index_mode;

			// Get previous index_mode
			const prevIndexMode = prevIndexModes.current.get(locationId);

			// Update tracked index_mode
			prevIndexModes.current.set(locationId, newIndexMode);

			// Check if index_mode changed from "none" to something else
			// This means the user just enabled indexing
			if (prevIndexMode === "none" && newIndexMode !== "none") {
				console.log(
					`[useLocationChangeInvalidation] Location ${locationId} indexing enabled (${prevIndexMode} -> ${newIndexMode}), invalidating directory_listing queries`,
				);

				// Invalidate all directory_listing queries
				// They will refetch and get data from the persistent index instead of ephemeral
				queryClient.invalidateQueries({
					predicate: (query) => {
						const key = query.queryKey;
						return (
							Array.isArray(key) &&
							key[0] === "query:files.directory_listing"
						);
					},
				});
			}
		};

		client
			.subscribeFiltered(
				{
					resource_type: "location",
					library_id: libraryId,
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
	}, [client, queryClient, libraryId]);
}
