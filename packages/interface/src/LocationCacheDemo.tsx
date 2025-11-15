/**
 * Demo component to test normalized cache with Locations
 *
 * This demonstrates:
 * - Cache hits on subsequent queries (no network)
 * - Automatic UI updates when ResourceChanged events arrive
 * - Instant updates across components
 */

import { useState } from "react";
import { useNormalizedCache, useCoreMutation } from "./context";
import type { Location } from "@sd/ts-client";
import { Button } from "@sd/ui";

export function LocationCacheDemo() {
	const [testName, setTestName] = useState("");

	// Use normalized cache - automatic updates when events arrive!
	// This is just TanStack Query + event listeners that call setQueryData atomically
	const locationsQuery = useNormalizedCache<{}, Location>({
		wireMethod: "query:locations.list",
		input: null, // Unit struct
		resourceType: "location",
	});

	const locations = locationsQuery.data;

	// Mutation to create location
	const createLocation = useCoreMutation("locations.add");

	const handleCreate = async () => {
		if (!testName.trim()) {
			alert("Enter a location name");
			return;
		}

		try {
			await createLocation.mutateAsync({
				path: {
					Physical: {
						device_slug: "local",
						path: `/tmp/test-${Date.now()}`,
					},
				},
				name: testName,
				mode: "Content",
			});

			setTestName("");

			// NOTE: No manual refetch needed!
			// When the backend emits ResourceChanged event:
			// 1. Cache receives event
			// 2. Updates entity store
			// 3. Notifies this query
			// 4. Component re-renders
			// 5. New location appears instantly!
		} catch (e: any) {
			console.error("Failed to create location:", e);
			alert("Failed to create location: " + e.message);
		}
	};

	return (
		<div className="flex flex-col gap-4 p-4 bg-app rounded-lg">
			<div className="flex flex-col gap-2">
				<h2 className="text-lg font-semibold text-ink">Normalized Cache Demo - Locations</h2>
				<p className="text-sm text-ink-dull">
					Create a location and watch it appear instantly without refetching!
				</p>
			</div>

			{/* Create location form */}
			<div className="flex gap-2">
				<input
					type="text"
					value={testName}
					onChange={(e) => setTestName(e.target.value)}
					placeholder="Location name..."
					className="flex-1 px-3 py-2 bg-app-box border border-app-line rounded-md text-ink"
					onKeyDown={(e) => {
						if (e.key === "Enter") handleCreate();
					}}
				/>
				<Button
					onClick={handleCreate}
					disabled={createLocation.isPending || !testName.trim()}
				>
					{createLocation.isPending ? "Creating..." : "Create Location"}
				</Button>
			</div>

			{/* Location list */}
			<div className="flex flex-col gap-2">
				<div className="flex items-center justify-between">
					<h3 className="text-sm font-medium text-ink">Locations</h3>
					<div className="flex items-center gap-2">
						{locationsQuery.isLoading && (
							<span className="text-xs text-ink-dull">Loading...</span>
						)}
						{locationsQuery.isFetching && !locationsQuery.isLoading && (
							<span className="text-xs text-ink-faint">Refetching...</span>
						)}
						<span className="text-xs text-ink-faint">
							{locations?.length || 0} locations
						</span>
					</div>
				</div>

				{locationsQuery.error && (
					<div className="px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-md text-red-500 text-sm">
						Error: {(locationsQuery.error as Error).message}
					</div>
				)}

				<div className="flex flex-col gap-1">
					{locations?.length === 0 && !locationsQuery.isLoading && (
						<div className="px-3 py-2 bg-app-box border border-app-line rounded-md text-ink-dull text-sm text-center">
							No locations yet. Create one above!
						</div>
					)}

					{locations?.map((location) => (
						<div
							key={location.id}
							className="flex items-center justify-between px-3 py-2 bg-app-box border border-app-line rounded-md hover:bg-app-hover transition-colors"
						>
							<div className="flex flex-col gap-0.5">
								<span className="text-sm font-medium text-ink">{location.name}</span>
								<span className="text-xs text-ink-dull">
									{location.sd_path.Physical?.path ||
									 location.sd_path.Cloud?.path ||
									 "Unknown path"}
								</span>
							</div>
							<div className="flex items-center gap-2">
								<span className="text-xs text-ink-faint">
									{location.file_count} files
								</span>
								<span className="text-xs text-ink-faint">•</span>
								<span className="text-xs text-ink-faint">
									{location.scan_state.Idle && "Idle"}
									{location.scan_state.Scanning && `Scanning ${location.scan_state.Scanning.progress}%`}
									{location.scan_state.Completed && "Completed"}
									{location.scan_state.Failed && "Failed"}
								</span>
							</div>
						</div>
					))}
				</div>
			</div>

			{/* TanStack Query stats */}
			<div className="px-3 py-2 bg-sidebar-box border border-sidebar-line rounded-md">
				<div className="text-xs text-sidebar-ink-dull space-y-1">
					<div className="font-medium text-sidebar-ink">Query State</div>
					<div>Status: {locationsQuery.status}</div>
					<div>Is Fetching: {locationsQuery.isFetching ? "Yes" : "No"}</div>
					<div>Data Updated At: {locationsQuery.dataUpdatedAt ? new Date(locationsQuery.dataUpdatedAt).toLocaleTimeString() : "Never"}</div>
					<div className="pt-1 mt-1 border-t border-sidebar-line text-sidebar-ink-faint">
						💡 Try creating a location - it will appear instantly without refetching
						because the ResourceChanged event updates TanStack Query's cache atomically!
					</div>
				</div>
			</div>
		</div>
	);
}
