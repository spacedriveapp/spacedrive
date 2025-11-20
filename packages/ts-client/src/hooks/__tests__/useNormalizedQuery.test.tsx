/**
 * useNormalizedQuery Event Replay Tests
 *
 * Tests the normalized query cache using real backend event data from fixtures.
 * Validates that events are correctly filtered and applied to maintain accurate cache state.
 */

import "./setup"; // Initialize DOM environment
import { describe, it, expect, beforeEach } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import {
	filterBatchResources,
	updateBatchResources,
	type UseNormalizedQueryOptions,
} from "../useNormalizedQuery";
import fixtures from "../../__fixtures__/backend_events.json";

describe("useNormalizedQuery - Event Replay Tests", () => {
	let queryClient: QueryClient;

	beforeEach(() => {
		queryClient = new QueryClient({
			defaultOptions: {
				queries: { retry: false, gcTime: Infinity },
			},
		});
	});

	it("should filter batch events to direct children only (exact mode) - PROVES BUG IS FIXED", async () => {
		const testCase = fixtures.test_cases.find(
			(t) => t.name === "directory_view_exact_mode",
		)!;

		expect(testCase).toBeDefined();

		// This test proves the subdirectory bug is fixed by testing the filtering logic directly
		// The filtering logic from useNormalizedQuery.updateBatchResources

		// Get the batch event - it contains MIXED files (direct + subdirectory)
		const batchEvent = testCase.events[0];
		const resources = (batchEvent as any).ResourceChangedBatch.resources;

		// Verify the batch contains both direct children AND subdirectory files
		const batchFileNames = resources.map((r: any) => r.name);
		expect(batchFileNames).toContain("direct_child1"); // Direct child ✓
		expect(batchFileNames).toContain("direct_child2"); // Direct child ✓
		expect(batchFileNames).toContain("grandchild1"); // Subdirectory file (should be filtered)
		expect(batchFileNames).toContain("grandchild2"); // Subdirectory file (should be filtered)

		// Use the ACTUAL production function from useNormalizedQuery
		const filteredResources = filterBatchResources(
			resources,
			testCase.query as UseNormalizedQueryOptions<any>,
		);

		// PROOF: Only 2 direct children should pass the filter
		console.log(
			"[Test] Filtered",
			resources.length,
			"→",
			filteredResources.length,
			"files",
		);
		expect(filteredResources).toHaveLength(2);

		const filteredNames = filteredResources.map((r: any) => r.name);
		expect(filteredNames).toContain("direct_child1");
		expect(filteredNames).toContain("direct_child2");
		expect(filteredNames).not.toContain("grandchild1"); // ✓ Filtered out!
		expect(filteredNames).not.toContain("grandchild2"); // ✓ Filtered out!
		expect(filteredNames).not.toContain("deep_file"); // ✓ Filtered out!

		// Now apply the filtered resources to a cache using the ACTUAL production function
		const testQueryClient = new QueryClient();
		const queryKey = [
			testCase.query.wireMethod,
			"test-library-id",
			testCase.query.input,
		];

		// Set initial state
		testQueryClient.setQueryData(queryKey, testCase.initial_state);

		// Call the ACTUAL updateBatchResources function from useNormalizedQuery
		updateBatchResources(
			resources, // Original batch with 5 files
			(batchEvent as any).ResourceChangedBatch.metadata,
			testCase.query as UseNormalizedQueryOptions<any>,
			queryKey,
			testQueryClient,
		);

		// Verify final cache state
		const finalCacheState = testQueryClient.getQueryData(queryKey) as any;
		console.log(
			"[Test] Final cache has",
			finalCacheState.files.length,
			"files",
		);

		expect(finalCacheState.files).toHaveLength(2);
		expect(finalCacheState.files.map((f: any) => f.name)).toContain(
			"direct_child1",
		);
		expect(finalCacheState.files.map((f: any) => f.name)).toContain(
			"direct_child2",
		);
		expect(finalCacheState.files.map((f: any) => f.name)).not.toContain(
			"grandchild1",
		);
		expect(finalCacheState.files.map((f: any) => f.name)).not.toContain(
			"grandchild2",
		);
		expect(finalCacheState.files.map((f: any) => f.name)).not.toContain(
			"deep_file",
		);

		// This proves the subdirectory bug is fixed ✓
		// The ACTUAL production updateBatchResources function:
		// - Filtered 5 files → 2 files
		// - Updated cache to contain only direct children
		// - Subtree completely excluded from final cache state
	});

	it("should include all descendants in recursive mode", () => {
		const testCase = fixtures.test_cases.find(
			(t) => t.name === "media_view_recursive_mode",
		)!;

		expect(testCase).toBeDefined();

		// Recursive mode doesn't filter by parent directory
		// All files under the path scope should be included
		const batchEvent = testCase.events[0];
		const resources = (batchEvent as any).ResourceChangedBatch.resources;

		// With includeDescendants: true, no client-side filtering happens
		const filteredResources = filterBatchResources(resources, {
			...testCase.query,
			includeDescendants: true,
		} as UseNormalizedQueryOptions<any>);

		// All files should pass through (no filtering for recursive mode)
		expect(filteredResources.length).toBe(resources.length);
	});

	it("should handle location events (no path filtering)", () => {
		const testCase = fixtures.test_cases.find(
			(t) => t.name === "location_updates",
		)!;

		expect(testCase).toBeDefined();
		expect(testCase.events).toHaveLength(1); // Should have captured location created event

		const locationEvent = testCase.events[0];

		// Verify it's a location ResourceChanged event
		expect((locationEvent as any).ResourceChanged).toBeDefined();
		expect((locationEvent as any).ResourceChanged.resource_type).toBe(
			"location",
		);

		// Location events have no affected_paths (global resources)
		const metadata = (locationEvent as any).ResourceChanged.metadata;
		if (metadata) {
			expect(metadata.affected_paths).toEqual([]);
		}

		// Verify the location resource is complete
		const location = (locationEvent as any).ResourceChanged.resource;
		expect(location.id).toBeDefined();
		expect(location.name).toBe("Test Location");

		// This validates that non-path-filtered resources work correctly
		// Locations, tags, albums, etc. use simpler event handling without path complexity
	});
});

describe("useNormalizedQuery - Client-Side Filtering", () => {
	it("should filter batch resources by pathScope", () => {
		const resources = [
			{
				id: "1",
				name: "direct_child",
				sd_path: {
					Physical: {
						device_slug: "test-mac",
						path: "/Desktop/direct_child.txt",
					},
				},
			},
			{
				id: "2",
				name: "subdirectory_file",
				sd_path: {
					Physical: {
						device_slug: "test-mac",
						path: "/Desktop/Subfolder/file.txt",
					},
				},
			},
		];

		const pathScope = {
			Physical: {
				device_slug: "test-mac",
				path: "/Desktop",
			},
		};

		// Filter logic (extracted from updateBatchResources)
		const filtered = resources.filter((resource) => {
			const filePath = resource.sd_path;
			if (!filePath?.Physical) return false;

			const pathStr = filePath.Physical.path;
			const scopeStr = pathScope.Physical.path;

			const lastSlash = pathStr.lastIndexOf("/");
			if (lastSlash === -1) return false;
			const parentDir = pathStr.substring(0, lastSlash);

			return parentDir === scopeStr;
		});

		expect(filtered).toHaveLength(1);
		expect(filtered[0].name).toBe("direct_child");
	});
});
