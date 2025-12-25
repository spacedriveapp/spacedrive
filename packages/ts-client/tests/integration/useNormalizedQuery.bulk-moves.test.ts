import "./setup"; // Ensure DOM environment is loaded first
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
import { SpacedriveProvider } from "../../src/hooks/useClient";
import { useNormalizedQuery } from "../../src/hooks/useNormalizedQuery";
import { renderHook, waitFor, act, cleanup } from "@testing-library/react";
import React from "react";

// Bridge config type matching Rust TestBridgeConfig
interface BridgeConfig {
	socket_addr: string;
	library_id: string;
	location_db_id: number;
	location_path: string;
	test_data_path: string;
}

describe("useNormalizedQuery - Bulk Moves Integration", () => {
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
			return originalCreateSubscription.call(
				this,
				filter,
				wrappedCallback,
			);
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

	afterEach(cleanup); // Clean up React Testing Library after each test

	test("should update cache when moving 20 files from subfolder to root", async () => {
		const rootPath = bridgeConfig.location_path;
		const subfolderPath = join(rootPath, "bulk_test");

		// Get device slug from hostname
		const deviceSlug = hostname().toLowerCase().replace(/\s+/g, "-");

		// Create wrapper for React hooks with SpacedriveProvider
		const wrapper = ({ children }: { children: React.ReactNode }) =>
			React.createElement(SpacedriveProvider, { client }, children);

		// Query root directory listing
		const { result: rootResult } = renderHook(
			() =>
				useNormalizedQuery({
					wireMethod: "query:files.directory_listing",
					input: {
						path: {
							Physical: {
								device_slug: deviceSlug,
								path: rootPath,
							},
						},
						sort_by: "name",
					},
					resourceType: "file",
					pathScope: {
						Physical: {
							device_slug: deviceSlug,
							path: rootPath,
						},
					},
					includeDescendants: false,
					debug: true,
				}),
			{ wrapper },
		);

		// Query subfolder listing
		const { result: subfolderResult } = renderHook(
			() =>
				useNormalizedQuery({
					wireMethod: "query:files.directory_listing",
					input: {
						path: {
							Physical: {
								device_slug: deviceSlug,
								path: subfolderPath,
							},
						},
						sort_by: "name",
					},
					resourceType: "file",
					pathScope: {
						Physical: {
							device_slug: deviceSlug,
							path: subfolderPath,
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
			expect(subfolderResult.current.data).toBeDefined();
		});

		const initialRootData = rootResult.current.data as {
			files: any[];
		};
		const initialSubfolderData = subfolderResult.current.data as {
			files: any[];
		};

		console.log(
			"[TS] Initial root file count:",
			initialRootData.files.length,
		);
		console.log(
			"[TS] Initial subfolder file count:",
			initialSubfolderData.files.length,
		);

		// Verify subfolder has 20 files
		expect(initialSubfolderData.files.length).toBeGreaterThanOrEqual(20);

		// Check for content-addressed files (files with content identity)
		const contentAddressedFiles = initialSubfolderData.files.filter(
			(f: any) =>
				f.kind === "File" &&
				f.sd_path?.Content &&
				f.alternate_paths?.length > 0,
		);
		const physicalOnlyFiles = initialSubfolderData.files.filter(
			(f: any) => f.kind === "File" && f.sd_path?.Physical,
		);

		console.log(
			"[TS] Content-addressed files (sd_path.Content):",
			contentAddressedFiles.length,
		);
		console.log(
			"[TS] Physical-only files (sd_path.Physical):",
			physicalOnlyFiles.length,
		);

		// Log example paths for debugging
		if (contentAddressedFiles.length > 0) {
			const example = contentAddressedFiles[0];
			console.log("[TS] Example content-addressed file:", {
				name: example.name,
				sd_path: example.sd_path,
				alternate_paths: example.alternate_paths,
			});
		}

		// CRITICAL: We need BOTH types to properly test the bug
		// Content-addressed files expose the cache update bug
		if (contentAddressedFiles.length === 0) {
			console.warn(
				"[TS] ⚠️  WARNING: No content-addressed files found! This test won't catch the production bug.",
			);
			console.warn(
				"[TS] ⚠️  The cache update bug only affects files with sd_path.Content + alternate_paths.",
			);
		}

		// Store initial file names from subfolder
		const fileNames = initialSubfolderData.files
			.filter((f: any) => f.kind === "File")
			.slice(0, 20)
			.map((f: any) => `${f.name}.${f.extension}`);

		console.log(
			"[TS] Moving 20 files from subfolder to root:",
			fileNames.slice(0, 5),
			"...",
		);

		// Move all 20 files
		for (const fileName of fileNames) {
			await rename(
				join(subfolderPath, fileName),
				join(rootPath, fileName),
			);
		}

		// Wait for watcher to detect and process all moves
		await new Promise((resolve) => setTimeout(resolve, 10000));

		// Verify cache updated correctly
		const finalRootData = rootResult.current.data as {
			files: any[];
		};
		const finalSubfolderData = subfolderResult.current.data as {
			files: any[];
		};

		const initialRootFileCount = initialRootData.files.filter(
			(f: any) => f.kind === "File",
		).length;
		const finalRootFileCount = finalRootData.files.filter(
			(f: any) => f.kind === "File",
		).length;
		const initialSubfolderFileCount = initialSubfolderData.files.filter(
			(f: any) => f.kind === "File",
		).length;
		const finalSubfolderFileCount = finalSubfolderData.files.filter(
			(f: any) => f.kind === "File",
		).length;

		console.log(
			"[TS] Root files: before",
			initialRootFileCount,
			"→ after",
			finalRootFileCount,
		);
		console.log(
			"[TS] Subfolder files: before",
			initialSubfolderFileCount,
			"→ after",
			finalSubfolderFileCount,
		);

		// 1. Verify root gained exactly 20 files
		expect(finalRootFileCount).toBe(initialRootFileCount + 20);

		// 2. Verify subfolder lost exactly 20 files
		expect(finalSubfolderFileCount).toBe(initialSubfolderFileCount - 20);

		// 3. Verify all 20 moved files are in root with correct paths and UUIDs preserved
		const initialFileMap = new Map(
			initialSubfolderData.files
				.filter((f: any) => f.kind === "File")
				.map((f: any) => [f.name, f]),
		);

		let movedFilesVerified = 0;
		let contentAddressedMovedCount = 0;
		let physicalOnlyMovedCount = 0;

		for (const fileName of fileNames) {
			const nameWithoutExt = fileName.split(".")[0];

			// Find in final root
			const fileInRoot = finalRootData.files.find(
				(f: any) => f.name === nameWithoutExt && f.kind === "File",
			);

			// Find in final subfolder (should NOT be there)
			const fileInSubfolder = finalSubfolderData.files.find(
				(f: any) => f.name === nameWithoutExt && f.kind === "File",
			);

			// Get original file for UUID comparison
			const originalFile = initialFileMap.get(nameWithoutExt);

			if (fileInRoot && !fileInSubfolder && originalFile) {
				// Verify UUID is preserved (proves it's a move, not delete+create)
				expect(fileInRoot.id).toBe(originalFile.id);

				// Track what type of file was moved successfully
				if (fileInRoot.sd_path?.Content) {
					contentAddressedMovedCount++;

					// For content-addressed files, check alternate_paths
					expect(fileInRoot.alternate_paths).toBeDefined();
					expect(fileInRoot.alternate_paths.length).toBeGreaterThan(
						0,
					);

					const physicalPath = fileInRoot.alternate_paths.find(
						(p: any) => p.Physical,
					)?.Physical?.path;
					expect(physicalPath).toBeDefined();
					expect(physicalPath).toContain(rootPath);
					expect(physicalPath).not.toContain("bulk_test");
				} else if (fileInRoot.sd_path?.Physical) {
					physicalOnlyMovedCount++;

					// For physical-only files, check sd_path directly
					expect(fileInRoot.sd_path.Physical.path).toContain(
						rootPath,
					);
					expect(fileInRoot.sd_path.Physical.path).not.toContain(
						"bulk_test",
					);
					expect(fileInRoot.sd_path.Physical.path).toContain(
						fileName,
					);
				}

				movedFilesVerified++;
			} else {
				console.error(`[TS] ❌ File ${fileName} verification failed:`, {
					inRoot: !!fileInRoot,
					inSubfolder: !!fileInSubfolder,
					hasOriginal: !!originalFile,
					originalType: originalFile?.sd_path?.Content
						? "Content"
						: "Physical",
				});

				// Extra debugging for content-addressed files
				if (originalFile?.sd_path?.Content) {
					console.error(
						`[TS] ⚠️  This was a content-addressed file - the cache update bug!`,
					);
				}
			}
		}

		console.log(
			"[TS] Verified",
			movedFilesVerified,
			"/ 20 files moved correctly",
		);
		console.log(
			"[TS]   - Content-addressed files moved:",
			contentAddressedMovedCount,
		);
		console.log(
			"[TS]   - Physical-only files moved:",
			physicalOnlyMovedCount,
		);

		expect(movedFilesVerified).toBe(20);

		// This assertion will FAIL before the bug fix if any content-addressed files were present
		// After the fix, this ensures content-addressed files are handled correctly
		if (contentAddressedFiles.length > 0) {
			console.log(
				"[TS] ✓ Content-addressed files were successfully moved (bug fix verified)",
			);
		}

		// 4. Verify no duplicates - files should not appear in both locations
		const rootFileNames = new Set(
			finalRootData.files
				.filter((f: any) => f.kind === "File")
				.map((f: any) => f.name),
		);
		const subfolderFileNames = new Set(
			finalSubfolderData.files
				.filter((f: any) => f.kind === "File")
				.map((f: any) => f.name),
		);

		let duplicateCount = 0;
		for (const fileName of fileNames) {
			const nameWithoutExt = fileName.split(".")[0];
			if (
				rootFileNames.has(nameWithoutExt) &&
				subfolderFileNames.has(nameWithoutExt)
			) {
				console.error(
					`[TS] ❌ Duplicate found: ${fileName} appears in both locations!`,
				);
				duplicateCount++;
			}
		}

		expect(duplicateCount).toBe(0);

		console.log(
			"[TS] ✓ Bulk file move detected and cache updated correctly",
		);
		console.log("[TS] ✓ All files have correct paths and preserved UUIDs");
		console.log("[TS] ✓ No duplicates found");
	}, 45000); // 45s timeout for bulk operations
});
