/**
 * TypeScript Integration Test: useNormalizedQuery with Folder Renames
 *
 * This test is spawned by a Rust test harness that provides:
 * - Real Spacedrive daemon running on Unix socket
 * - Indexed location with test folders
 * - Connection configuration via BRIDGE_CONFIG_PATH env var
 *
 * Test flow:
 * 1. Connect to daemon using bridge config
 * 2. Query directory listing with useNormalizedQuery
 * 3. Rename folder in filesystem
 * 4. Verify cache updates correctly and children remain accessible
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
import { readFile } from "fs/promises";
import { rename } from "fs/promises";
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

describe("useNormalizedQuery - Folder Rename Integration", () => {
	test("should update cache when folder is renamed", async () => {
		const locationPath = bridgeConfig.location_path;
		const originalPath = join(locationPath, "original_folder");
		const renamedPath = join(locationPath, "renamed_folder");

		// Get device slug from hostname
		const deviceSlug = hostname().toLowerCase().replace(/\s+/g, "-");

		// Create wrapper for React hooks with SpacedriveProvider
		const wrapper = ({ children }: { children: React.ReactNode }) =>
			React.createElement(SpacedriveProvider, { client }, children);

		// Query root directory listing (should contain original_folder)
		const { result: rootResult } = renderHook(
			() =>
				useNormalizedQuery({
					wireMethod: "query:files.directory_listing",
					input: {
						path: {
							Physical: {
								device_slug: deviceSlug,
								path: locationPath,
							},
						},
						sort_by: "name",
					},
					resourceType: "file",
					pathScope: {
						Physical: {
							device_slug: deviceSlug,
							path: locationPath,
						},
					},
					includeDescendants: false,
					debug: true,
				}),
			{ wrapper },
		);

		// Query original_folder contents
		const { result: folderResult } = renderHook(
			() =>
				useNormalizedQuery({
					wireMethod: "query:files.directory_listing",
					input: {
						path: {
							Physical: {
								device_slug: deviceSlug,
								path: originalPath,
							},
						},
						sort_by: "name",
					},
					resourceType: "file",
					pathScope: {
						Physical: {
							device_slug: deviceSlug,
							path: originalPath,
						},
					},
					includeDescendants: false,
					debug: true,
				}),
			{ wrapper },
		);

		// Wait for initial data
		await waitFor(() => {
			expect(rootResult.current.data).toBeDefined();
			expect(folderResult.current.data).toBeDefined();
		});

		console.log("[TS] Initial root listing:", rootResult.current.data);
		console.log(
			"[TS] Initial original_folder contents:",
			folderResult.current.data,
		);

		// Verify initial state
		const initialRootData = rootResult.current.data as { files: any[] };
		const originalFolder = initialRootData.files.find(
			(f: any) => f.name === "original_folder" && f.kind === "Directory",
		);
		expect(originalFolder).toBeDefined();

		const originalFolderUuid = originalFolder.uuid;
		const originalFolderChildrenData = folderResult.current.data as {
			files: any[];
		};
		expect(originalFolderChildrenData.files.length).toBeGreaterThanOrEqual(
			2,
		); // file1.txt, file2.rs

		// Rename the folder
		console.log("[TS] Renaming folder: original_folder -> renamed_folder");
		await rename(originalPath, renamedPath);

		// Wait for watcher to detect and emit events
		await new Promise((resolve) => setTimeout(resolve, 8000));

		// Verify cache updated correctly
		const finalRootData = rootResult.current.data as { files: any[] };
		console.log("[TS] Final root listing:", finalRootData);

		// Original folder should no longer exist in root
		const originalStillExists = finalRootData.files.find(
			(f: any) => f.name === "original_folder" && f.kind === "Directory",
		);
		expect(originalStillExists).toBeUndefined();

		// Renamed folder should exist in root
		const renamedFolder = finalRootData.files.find(
			(f: any) => f.name === "renamed_folder" && f.kind === "Directory",
		);
		expect(renamedFolder).toBeDefined();

		// UUID should be preserved (folder identity maintained)
		expect(renamedFolder.uuid).toBe(originalFolderUuid);

		console.log(
			"[TS] ✓ Folder rename detected and cache updated correctly",
		);
		console.log("[TS] ✓ Folder UUID preserved:", renamedFolder.uuid);
	}, 30000); // 30s timeout for watcher delays
});
