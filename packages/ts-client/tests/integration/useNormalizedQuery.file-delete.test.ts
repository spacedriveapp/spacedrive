/**
 * TypeScript Integration Test: useNormalizedQuery with File Deletion
 *
 * This test is spawned by a Rust test harness that provides:
 * - Real Spacedrive daemon running on Unix socket
 * - Indexed location with test files
 * - Connection configuration via BRIDGE_CONFIG_PATH env var
 *
 * Test flow:
 * 1. Connect to daemon using bridge config
 * 2. Query directory listing with useNormalizedQuery
 * 3. Delete files from filesystem
 * 4. Verify cache updates correctly and files are removed from listing
 */

// Setup DOM environment before any other imports
import "./setup";

import {
	describe,
	test,
	expect,
	beforeAll,
	afterAll,
	afterEach,
} from "bun:test";
import { readFile, unlink } from "fs/promises";
import { join } from "path";
import { hostname } from "os";
import { SpacedriveClient } from "../../src/client";
import { renderHook, waitFor, cleanup } from "@testing-library/react";
import { SpacedriveProvider } from "../../src/hooks/useClient";
import { useNormalizedQuery } from "../../src/hooks/useNormalizedQuery";
import React from "react";

// Bridge configuration from Rust test harness
interface BridgeConfig {
	socket_addr: string;
	library_id: string;
	location_db_id: number;
	location_path: string;
	test_data_path: string;
}

let bridgeConfig: BridgeConfig;
let client: SpacedriveClient;
const allEventsReceived: any[] = []; // Collect all events for debugging

beforeAll(async () => {
	// Read bridge config from path provided by Rust test
	const configPath = process.env.BRIDGE_CONFIG_PATH;
	if (!configPath) {
		throw new Error("BRIDGE_CONFIG_PATH environment variable not set");
	}

	console.log(`[TS] Reading bridge config from: ${configPath}`);
	const configJson = await readFile(configPath, "utf-8");
	bridgeConfig = JSON.parse(configJson);

	console.log(`[TS] Bridge config:`, bridgeConfig);

	// Connect to daemon via TCP socket
	client = SpacedriveClient.fromTcpSocket(bridgeConfig.socket_addr);

	console.log(`[TS] Connected to daemon`);

	// Set library context
	client.setCurrentLibrary(bridgeConfig.library_id);
	console.log(`[TS] Library set to: ${bridgeConfig.library_id}`);

	// Hook into the subscription manager to collect all events
	const originalCreateSubscription = (client as any).subscriptionManager
		.createSubscription;
	(client as any).subscriptionManager.createSubscription = function (
		filter: any,
		callback: any,
	) {
		const wrappedCallback = (event: any) => {
			allEventsReceived.push({
				timestamp: new Date().toISOString(),
				filter,
				event,
			});
			console.log(
				`[TS] 🔔 Event received:`,
				JSON.stringify(event, null, 2),
			);
			callback(event);
		};
		return originalCreateSubscription.call(this, filter, wrappedCallback);
	};
});

afterAll(async () => {
	// Log all events at the end for debugging
	console.log(
		`[TS] ===== ALL EVENTS RECEIVED (${allEventsReceived.length}) =====`,
	);
	allEventsReceived.forEach((item, idx) => {
		console.log(`[TS] Event ${idx + 1} at ${item.timestamp}:`);
		console.log(`[TS]   Filter:`, JSON.stringify(item.filter, null, 2));
		console.log(`[TS]   Event:`, JSON.stringify(item.event, null, 2));
	});
	console.log(`[TS] ===== END OF EVENTS =====`);
	// No explicit disconnect needed for stateless transports
});

afterEach(() => {
	// Clean up React components after each test
	cleanup();
});

describe("useNormalizedQuery - File Deletion Integration", () => {
	test("should update cache when files are deleted", async () => {
		const locationPath = bridgeConfig.location_path;
		const deleteTestPath = join(locationPath, "delete_test");

		// Get device slug from hostname
		const deviceSlug = hostname().toLowerCase().replace(/\s+/g, "-");

		// Create wrapper for React hooks with SpacedriveProvider
		const wrapper = ({ children }: { children: React.ReactNode }) =>
			React.createElement(SpacedriveProvider, { client }, children);

		// Query delete_test directory listing
		const { result: folderResult } = renderHook(
			() =>
				useNormalizedQuery({
					wireMethod: "query:files.directory_listing",
					input: {
						path: {
							Physical: {
								device_slug: deviceSlug,
								path: deleteTestPath,
							},
						},
						sort_by: "name",
					},
					resourceType: "file",
					pathScope: {
						Physical: {
							device_slug: deviceSlug,
							path: deleteTestPath,
						},
					},
					includeDescendants: false,
					debug: true,
				}),
			{ wrapper },
		);

		// Wait for initial data
		await waitFor(() => {
			expect(folderResult.current.data).toBeDefined();
		});

		console.log(
			"[TS] Initial delete_test contents:",
			folderResult.current.data,
		);

		// Verify initial state
		const initialData = folderResult.current.data as { files: any[] };
		const initialFileCount = initialData.files.filter(
			(f: any) => f.kind === "File",
		).length;

		console.log("[TS] Initial file count:", initialFileCount);
		expect(initialFileCount).toBeGreaterThanOrEqual(3);

		// Get the files to delete
		const filesToDelete = initialData.files
			.filter((f: any) => f.kind === "File")
			.slice(0, 3);

		console.log("[TS] Files to delete:", JSON.stringify(filesToDelete, null, 2));

		const deletedFileUuids = filesToDelete.map((f: any) => f.id);
		const deletedFileNames = filesToDelete.map(
			(f: any) => `${f.name}.${f.extension}`,
		);

		console.log(
			"[TS] Deleting files:",
			deletedFileNames,
			"with UUIDs:",
			deletedFileUuids,
		);

		// Delete the files
		for (const fileName of deletedFileNames) {
			const filePath = join(deleteTestPath, fileName);
			console.log(`[TS] Deleting: ${filePath}`);
			try {
				await unlink(filePath);
				console.log(`[TS] Successfully deleted: ${filePath}`);
			} catch (error) {
				console.error(`[TS] Failed to delete ${filePath}:`, error);
				throw error;
			}
		}

		// Wait for watcher to detect and emit events
		await new Promise((resolve) => setTimeout(resolve, 8000));

		// Verify cache updated correctly
		const finalData = folderResult.current.data as { files: any[] };
		console.log("[TS] Final delete_test contents:", finalData);

		const finalFileCount = finalData.files.filter(
			(f: any) => f.kind === "File",
		).length;

		console.log(
			"[TS] File count: before",
			initialFileCount,
			"→ after",
			finalFileCount,
		);

		// Verify file count decreased by 3
		expect(finalFileCount).toBe(initialFileCount - 3);

		// Verify deleted files no longer exist in cache
		for (let i = 0; i < deletedFileNames.length; i++) {
			const fileName = deletedFileNames[i];
			const fileUuid = deletedFileUuids[i];
			const nameWithoutExt = fileName.split(".")[0];

			const fileStillExists = finalData.files.find(
				(f: any) => f.name === nameWithoutExt && f.kind === "File",
			);

			expect(fileStillExists).toBeUndefined();

			const fileStillExistsById = finalData.files.find(
				(f: any) => f.id === fileUuid,
			);

			expect(fileStillExistsById).toBeUndefined();

			console.log(`[TS] ✓ File ${fileName} (${fileUuid}) removed from cache`);
		}

		console.log("[TS] ✓ File deletions detected and cache updated correctly");
	}, 30000); // 30s timeout for watcher delays
});
