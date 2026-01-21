/**
 * TypeScript Integration Test: useNormalizedQuery with File Moves
 *
 * This test is spawned by a Rust test harness that provides:
 * - Real Spacedrive daemon running on Unix socket
 * - Indexed location with test files
 * - Connection configuration via BRIDGE_CONFIG_PATH env var
 *
 * Test flow:
 * 1. Connect to daemon using bridge config
 * 2. Query directory listing with useNormalizedQuery
 * 3. Move files in filesystem
 * 4. Verify cache updates correctly via WebSocket events
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

describe("useNormalizedQuery - File Moves Integration", () => {
	test("should update cache when file moves between folders", async () => {
		const folderAPath = join(bridgeConfig.location_path, "folder_a");
		const folderBPath = join(bridgeConfig.location_path, "folder_b");

		// Get device slug from hostname
		const deviceSlug = hostname().toLowerCase().replace(/\s+/g, "-");

		// Create wrapper for React hooks with SpacedriveProvider
		const wrapper = ({ children }: { children: React.ReactNode }) =>
			React.createElement(SpacedriveProvider, { client }, children);

		// Query folder_a listing
		const { result: folderAResult } = renderHook(
			() =>
				useNormalizedQuery({
					wireMethod: "query:files.directory_listing",
					input: {
						path: {
							Physical: {
								device_slug: deviceSlug,
								path: folderAPath,
							},
						},
						sort_by: "name",
					},
					resourceType: "file",
					pathScope: {
						Physical: {
							device_slug: deviceSlug,
							path: folderAPath,
						},
					},
					includeDescendants: false,
					debug: true,
				}),
			{ wrapper },
		);

		// Query folder_b listing
		const { result: folderBResult } = renderHook(
			() =>
				useNormalizedQuery({
					wireMethod: "query:files.directory_listing",
					input: {
						path: {
							Physical: {
								device_slug: deviceSlug,
								path: folderBPath,
							},
						},
						sort_by: "name",
					},
					resourceType: "file",
					pathScope: {
						Physical: {
							device_slug: deviceSlug,
							path: folderBPath,
						},
					},
					includeDescendants: false,
					debug: true,
				}),
			{ wrapper },
		);

		// Wait for initial data
		await waitFor(
			() => {
				console.log("[TS] Waiting for data...", {
					folderA: {
						data: folderAResult.current.data,
						error: folderAResult.current.error,
						isLoading: folderAResult.current.isLoading,
					},
					folderB: {
						data: folderBResult.current.data,
						error: folderBResult.current.error,
						isLoading: folderBResult.current.isLoading,
					},
				});
				expect(folderAResult.current.data).toBeDefined();
				expect(folderBResult.current.data).toBeDefined();
			},
			{ timeout: 5000 },
		);

		console.log("[TS] Initial folder_a files:", folderAResult.current.data);
		console.log("[TS] Initial folder_b files:", folderBResult.current.data);

		// Verify initial state
		const initialFolderAData = folderAResult.current.data as {
			files: any[];
		};
		const initialFolderBData = folderBResult.current.data as {
			files: any[];
		};

		expect(initialFolderAData.files.length).toBeGreaterThanOrEqual(2); // file1.txt, file2.rs
		expect(initialFolderBData.files.length).toBeGreaterThanOrEqual(1); // file3.md

		const file1Before = initialFolderAData.files.find(
			(f: any) => f.name === "file1",
		);
		expect(file1Before).toBeDefined();

		// Move file1.txt from folder_a to folder_b
		console.log("[TS] Moving file1.txt from folder_a to folder_b");
		await rename(
			join(folderAPath, "file1.txt"),
			join(folderBPath, "file1.txt"),
		);

		// Wait for watcher to detect and emit events (watcher buffers for 500ms + tick time)
		await new Promise((resolve) => setTimeout(resolve, 8000));

		// Verify cache updated correctly
		const finalFolderAData = folderAResult.current.data as { files: any[] };
		const finalFolderBData = folderBResult.current.data as { files: any[] };

		console.log("[TS] Final folder_a files:", finalFolderAData);
		console.log("[TS] Final folder_b files:", finalFolderBData);

		// file1 should no longer be in folder_a
		const file1InFolderA = finalFolderAData.files.find(
			(f: any) => f.name === "file1",
		);
		expect(file1InFolderA).toBeUndefined();

		// file1 should now be in folder_b
		const file1InFolderB = finalFolderBData.files.find(
			(f: any) => f.name === "file1",
		);
		expect(file1InFolderB).toBeDefined();

		// UUID should be preserved (move detection)
		expect(file1InFolderB.uuid).toBe(file1Before.uuid);

		console.log("[TS] ✓ File move detected and cache updated correctly");
	}, 30000); // 30s timeout for watcher delays
});
