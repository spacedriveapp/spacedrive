import { describe, expect, test } from "bun:test";
import {
	shouldRefreshSourceForEvent,
	sourceFileEventFilter,
} from "./sourceRefresh";

describe("source refresh events", () => {
	test("scopes file subscriptions to the source root and library", () => {
		expect(sourceFileEventFilter("library", "/drive/photos")).toEqual({
			library_id: "library",
			resource_type: "file",
			event_types: [
				"ResourceChanged",
				"ResourceChangedBatch",
				"ResourceDeleted",
			],
			path_scope: {
				Physical: { device_slug: "local", path: "/drive/photos" },
			},
			include_descendants: true,
		});
		expect(
			sourceFileEventFilter("library", null).path_scope,
		).toBeUndefined();
	});

	test("refreshes file changes and pathless deletes, not other resources", () => {
		expect(
			shouldRefreshSourceForEvent({
				ResourceChanged: { resource_type: "file", resource: null },
			}),
		).toBe(true);
		expect(
			shouldRefreshSourceForEvent({
				ResourceChangedBatch: { resource_type: "file", resources: [] },
			}),
		).toBe(true);
		expect(
			shouldRefreshSourceForEvent({
				ResourceDeleted: { resource_type: "file", resource_id: "1" },
			}),
		).toBe(true);
		expect(
			shouldRefreshSourceForEvent({
				ResourceDeleted: { resource_type: "tag", resource_id: "1" },
			}),
		).toBe(false);
	});

	test("refreshes an absent source when its volume fingerprint is not known", () => {
		const mount = {
			VolumeMountChanged: { fingerprint: "returned", is_mounted: true },
		} as const;
		expect(
			shouldRefreshSourceForEvent(mount, undefined, "source-volume"),
		).toBe(true);
		expect(shouldRefreshSourceForEvent(mount, undefined, null)).toBe(false);
		expect(
			shouldRefreshSourceForEvent(mount, "other", "source-volume"),
		).toBe(false);
	});

	test("refreshes on source jobs and only the matching volume", () => {
		expect(
			shouldRefreshSourceForEvent({
				JobCompleted: {
					job_id: "1",
					job_type: "indexer",
					device_id: "d",
					output: { type: "Success" },
				},
			}),
		).toBe(true);
		expect(
			shouldRefreshSourceForEvent({
				JobCompleted: {
					job_id: "1",
					job_type: "source_sync",
					device_id: "d",
					output: { type: "Success" },
				},
			}),
		).toBe(true);
		expect(
			shouldRefreshSourceForEvent({
				JobCompleted: {
					job_id: "1",
					job_type: "other",
					device_id: "d",
					output: { type: "Success" },
				},
			}),
		).toBe(false);
		expect(
			shouldRefreshSourceForEvent(
				{ VolumeMountChanged: { fingerprint: "a", is_mounted: true } },
				"a",
			),
		).toBe(true);
		expect(
			shouldRefreshSourceForEvent(
				{ VolumeMountChanged: { fingerprint: "b", is_mounted: true } },
				"a",
			),
		).toBe(false);
	});
});
