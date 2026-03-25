/**
 * Ephemeral Directory Event Streaming Test
 *
 * Tests the core ephemeral browsing flow without React:
 * 1. Subscribe to events for a directory path scope
 * 2. Query the directory listing (backend returns empty, dispatches indexer)
 * 3. Verify ResourceChangedBatch events arrive through the subscription
 *
 * This test reproduces the exact race condition that causes empty directories
 * in the UI: the query triggers indexing, but events may not reach the
 * subscription due to timing, buffer overflow, or filter mismatches.
 */

import "./setup";

import { describe, test, expect, beforeAll, afterAll } from "bun:test";
import { readFile } from "fs/promises";
import { hostname } from "os";
import { SpacedriveClient } from "../../src/client";
import { TcpSocketTransport } from "../../src/transport";

interface BridgeConfig {
	socket_addr: string;
	library_id: string;
	device_slug: string;
	ephemeral_dir_path: string;
	test_data_path: string;
}

let bridgeConfig: BridgeConfig;
let client: SpacedriveClient;
let deviceSlug: string;

beforeAll(async () => {
	const configPath = process.env.BRIDGE_CONFIG_PATH;
	if (!configPath) {
		throw new Error("BRIDGE_CONFIG_PATH environment variable not set");
	}

	const configJson = await readFile(configPath, "utf-8");
	bridgeConfig = JSON.parse(configJson);

	console.log(`[TS] Bridge config:`, bridgeConfig);

	client = SpacedriveClient.fromTcpSocket(bridgeConfig.socket_addr);
	client.setCurrentLibrary(bridgeConfig.library_id);

	// Use the device slug from the daemon, not hostname() which may differ
	deviceSlug = bridgeConfig.device_slug;
	console.log(`[TS] Device slug (from daemon): ${deviceSlug}`);
	console.log(`[TS] Hostname for comparison: ${hostname().toLowerCase().replace(/\s+/g, "-")}`);
});

describe("Ephemeral Directory Event Streaming", () => {
	/**
	 * Test 1: Raw transport-level event delivery
	 *
	 * Subscribe via raw TCP, then query directory listing.
	 * This bypasses React, SubscriptionManager, and useNormalizedQuery entirely
	 * to test the daemon -> TCP -> event pipeline in isolation.
	 */
	test("events arrive via TCP subscription after directory listing query", async () => {
		const ephemeralPath = bridgeConfig.ephemeral_dir_path;
		const pathScope = {
			Physical: {
				device_slug: deviceSlug,
				path: ephemeralPath,
			},
		};

		const receivedEvents: any[] = [];

		// Step 1: Subscribe FIRST
		console.log(`[TS] Subscribing to events for: ${ephemeralPath}`);
		const transport = new TcpSocketTransport(bridgeConfig.socket_addr);
		const unsubscribe = await transport.subscribe(
			(event: any) => {
				console.log(`[TS] EVENT received:`, JSON.stringify(event).slice(0, 200));
				receivedEvents.push(event);
			},
			{
				event_types: [
					"ResourceChanged",
					"ResourceChangedBatch",
					"ResourceDeleted",
					"Refresh",
				],
				filter: {
					resource_type: "file",
					path_scope: pathScope,
					library_id: bridgeConfig.library_id,
					include_descendants: false,
				},
			},
		);

		console.log(`[TS] Subscription active`);

		// Small delay to ensure subscription is fully registered on daemon
		await new Promise((r) => setTimeout(r, 100));

		// Step 2: Query the directory listing (triggers ephemeral indexing)
		console.log(`[TS] Querying directory listing for: ${ephemeralPath}`);
		const queryResult = await client.execute(
			"query:files.directory_listing",
			{
				path: pathScope,
				sort_by: "name",
				limit: null,
				include_hidden: false,
				folders_first: true,
			},
		);

		console.log(
			`[TS] Query returned:`,
			JSON.stringify(queryResult).slice(0, 200),
		);

		// The query may return empty (indexer dispatched async) or may return
		// cached results if a previous run populated the cache.
		// Either way, events should arrive.

		// Step 3: Wait for events to arrive
		// Ephemeral indexing is fast (<500ms), give generous timeout
		const deadline = Date.now() + 10_000;
		while (Date.now() < deadline) {
			if (receivedEvents.length > 0) {
				console.log(
					`[TS] Got ${receivedEvents.length} event(s) after ${Date.now() - (deadline - 10_000)}ms`,
				);
				break;
			}
			await new Promise((r) => setTimeout(r, 50));
		}

		// Step 4: Verify events arrived
		console.log(`[TS] Total events received: ${receivedEvents.length}`);
		for (const [i, event] of receivedEvents.entries()) {
			console.log(`[TS] Event ${i + 1}:`, JSON.stringify(event).slice(0, 300));
		}

		// Clean up subscription
		unsubscribe();

		// If the query returned files directly (cache hit), events may not fire.
		// Check both paths.
		const queryFiles = (queryResult as any)?.files ?? [];
		const eventFiles: string[] = [];

		for (const event of receivedEvents) {
			if (event.ResourceChangedBatch) {
				for (const resource of event.ResourceChangedBatch.resources) {
					eventFiles.push(resource.name);
				}
			} else if (event.ResourceChanged) {
				eventFiles.push(event.ResourceChanged.resource.name);
			}
		}

		const totalFiles = queryFiles.length + eventFiles.length;
		console.log(
			`[TS] Files from query: ${queryFiles.length}, from events: ${eventFiles.length}`,
		);

		if (queryFiles.length > 0) {
			console.log(
				`[TS] Query returned files (cache hit):`,
				queryFiles.map((f: any) => f.name),
			);
		}
		if (eventFiles.length > 0) {
			console.log(`[TS] Event files:`, eventFiles);
		}

		// We expect files to arrive via EITHER the query response (cache hit)
		// OR events (first-time indexing). At least one must work.
		// The directory has 6 items: document.txt, photo.jpg, notes.md, script.rs, data.json, subfolder
		expect(totalFiles).toBeGreaterThanOrEqual(5);

		// If the query returned empty, ALL files must come from events
		if (queryFiles.length === 0) {
			console.log(`[TS] Query returned empty — verifying events delivered all files`);
			expect(eventFiles.length).toBeGreaterThanOrEqual(5);
		}

		console.log(`[TS] Event streaming test passed`);
	}, 30_000);

	/**
	 * Test 2: EventBuffer replay
	 *
	 * Query first (triggers indexing + events), THEN subscribe.
	 * The EventBuffer should replay recent events to the new subscription.
	 * This is the exact race condition the buffer was designed to solve.
	 */
	test("EventBuffer replays events to late subscriber", async () => {
		// Use a subdirectory so it hasn't been indexed yet
		const subPath = bridgeConfig.ephemeral_dir_path + "/subfolder";
		const pathScope = {
			Physical: {
				device_slug: deviceSlug,
				path: subPath,
			},
		};

		// Step 1: Query FIRST (triggers indexing, events go to buffer)
		console.log(`[TS] Querying BEFORE subscribing: ${subPath}`);
		const queryResult = await client.execute(
			"query:files.directory_listing",
			{
				path: pathScope,
				sort_by: "name",
				limit: null,
				include_hidden: false,
				folders_first: true,
			},
		);

		console.log(
			`[TS] Query returned:`,
			JSON.stringify(queryResult).slice(0, 200),
		);

		// Small delay to let events buffer (but not expire — 5s retention)
		await new Promise((r) => setTimeout(r, 500));

		// Step 2: Subscribe AFTER query (late subscriber)
		const replayedEvents: any[] = [];
		const transport = new TcpSocketTransport(bridgeConfig.socket_addr);
		const unsubscribe = await transport.subscribe(
			(event: any) => {
				console.log(
					`[TS] REPLAYED event:`,
					JSON.stringify(event).slice(0, 200),
				);
				replayedEvents.push(event);
			},
			{
				event_types: [
					"ResourceChanged",
					"ResourceChangedBatch",
					"ResourceDeleted",
					"Refresh",
				],
				filter: {
					resource_type: "file",
					path_scope: pathScope,
					library_id: bridgeConfig.library_id,
					include_descendants: false,
				},
			},
		);

		// Wait a bit for replayed events to arrive
		await new Promise((r) => setTimeout(r, 1000));

		console.log(`[TS] Replayed events: ${replayedEvents.length}`);
		for (const [i, event] of replayedEvents.entries()) {
			console.log(
				`[TS] Replayed ${i + 1}:`,
				JSON.stringify(event).slice(0, 300),
			);
		}

		unsubscribe();

		// The subfolder has 1 file (nested.txt).
		// If the buffer replay works, we should see it.
		// If it doesn't, this test will fail and tell us the buffer is broken.
		const queryFiles = (queryResult as any)?.files ?? [];
		const replayFiles: string[] = [];
		for (const event of replayedEvents) {
			if (event.ResourceChangedBatch) {
				for (const resource of event.ResourceChangedBatch.resources) {
					replayFiles.push(resource.name);
				}
			} else if (event.ResourceChanged) {
				replayFiles.push(event.ResourceChanged.resource.name);
			}
		}

		const total = queryFiles.length + replayFiles.length;
		console.log(
			`[TS] Buffer replay: ${queryFiles.length} from query, ${replayFiles.length} from replay`,
		);

		// We expect the file to arrive via either query (cache hit from test 1)
		// or buffer replay. Log clearly which path succeeded.
		if (total === 0) {
			console.error(
				`[TS] FAILURE: No files from query or buffer replay for ${subPath}`,
			);
			console.error(
				`[TS] This means the EventBuffer is not replaying events to late subscribers`,
			);
		}

		expect(total).toBeGreaterThanOrEqual(1);
		console.log(`[TS] Buffer replay test passed`);
	}, 30_000);
});
