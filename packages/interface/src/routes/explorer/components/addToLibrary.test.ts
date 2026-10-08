import { describe, expect, test } from "bun:test";
import type { SourceInfo, Volume } from "@sd/ts-client";
import {
	addRefusal,
	buildPayload,
	containingVolume,
	effectiveDefaults,
	inclusionOf,
	libraryDefaults,
	overridesFrom,
	placementChoices,
	type AddSettings,
} from "./addToLibrary";

function volume(overrides: Partial<Volume>): Volume {
	return {
		id: "vol",
		fingerprint: "fp",
		device_id: "dev",
		name: "Archive",
		library_id: null,
		is_tracked: false,
		mount_point: "/Volumes/Archive",
		mount_points: [],
		mount_type: "External",
		total_capacity: 0,
		available_space: 0,
		is_read_only: false,
		is_mounted: true,
		hardware_id: null,
		cloud_identifier: null,
		cloud_config: null,
		apfs_container: null,
		container_volume_id: null,
		path_mappings: [],
		is_user_visible: true,
		...overrides,
	} as Volume;
}

function source(root: string, overrides: Partial<SourceInfo> = {}): SourceInfo {
	return {
		id: root,
		name: root,
		data_type: "filesystem",
		adapter_id: null,
		item_count: 0,
		last_synced: null,
		status: "idle",
		root,
		volume_uuid: null,
		attached: true,
		total_bytes: null,
		last_seen_at: null,
		device_id: null,
		device_label: null,
		placement: "in_library",
		...overrides,
	};
}

const library: AddSettings = {
	placement: "in_library",
	keep_offline_copy: true,
	unfiltered: false,
	identify_content: true,
};

describe("library defaults", () => {
	test("fill unset fields with the core's defaults", () => {
		expect(libraryDefaults(undefined)).toEqual(library);
		expect(libraryDefaults({ placement: "on_source" })).toEqual({
			...library,
			placement: "on_source",
		});
	});
});

describe("effective defaults", () => {
	test("a folder takes the library defaults as they are", () => {
		const defaults = effectiveDefaults(
			library,
			{ kind: "path", path: "/Volumes/Archive/Photos" },
			volume({}),
			undefined,
		);
		expect(defaults).toEqual(library);
	});

	test("a whole external drive captures unfiltered, as volumes.track does", () => {
		const defaults = effectiveDefaults(
			library,
			{ kind: "volume", volume: volume({}) },
			volume({}),
			undefined,
		);
		expect(defaults.unfiltered).toBe(true);
	});

	test("an on_source default falls back to the library on a network volume", () => {
		const defaults = effectiveDefaults(
			{ ...library, placement: "on_source" },
			{ kind: "path", path: "/mnt/nas/share" },
			volume({ mount_type: "Network", mount_point: "/mnt/nas" }),
			undefined,
		);
		expect(defaults.placement).toBe("in_library");
	});

	test("a re-add starts from what the source saved", () => {
		const existing = source("/Volumes/Archive", {
			placement: "on_source",
			settings: {
				placement: "on_source",
				unfiltered: true,
				keep_offline_copy: false,
				identify_content: false,
			},
		});
		const defaults = effectiveDefaults(
			library,
			{ kind: "path", path: "/Volumes/Archive" },
			volume({}),
			existing,
		);
		expect(defaults).toEqual({
			placement: "on_source",
			unfiltered: true,
			keep_offline_copy: false,
			identify_content: false,
		});
	});
});

describe("placement choices", () => {
	test("on source needs a local writable drive", () => {
		expect(placementChoices(volume({}), undefined).on_source.available).toBe(true);
		expect(
			placementChoices(volume({ mount_type: "Network" }), undefined).on_source,
		).toMatchObject({ available: false });
		expect(
			placementChoices(volume({ cloud_identifier: "bucket" }), undefined).on_source,
		).toMatchObject({ available: false });
		expect(
			placementChoices(volume({ is_read_only: true }), undefined).on_source,
		).toMatchObject({ available: false });
		expect(placementChoices(undefined, undefined).on_source).toMatchObject({
			available: false,
		});
	});

	test("an existing catalog pins its placement", () => {
		const choices = placementChoices(
			volume({}),
			source("/Volumes/Archive", { placement: "on_source" }),
		);
		expect(choices.on_source.available).toBe(true);
		expect(choices.in_library.available).toBe(false);
	});
});

describe("overrides and payload", () => {
	test("unchanged settings send no overrides", () => {
		expect(overridesFrom(library, { ...library })).toEqual({});
	});

	test("only changed fields become overrides", () => {
		const chosen: AddSettings = {
			...library,
			placement: "on_source",
			identify_content: false,
		};
		expect(overridesFrom(library, chosen)).toEqual({
			placement: "on_source",
			identify_content: false,
		});
	});

	test("turning the external-drive capture off is sent explicitly", () => {
		const target = { kind: "volume" as const, volume: volume({}) };
		const defaults = effectiveDefaults(library, target, target.volume, undefined);
		const overrides = overridesFrom(defaults, { ...defaults, unfiltered: false });
		expect(buildPayload(target, "  Archive ", overrides)).toEqual({
			type: "volumes.track",
			input: {
				fingerprint: "fp",
				display_name: "Archive",
				overrides: { unfiltered: false },
			},
		});
	});

	test("a folder goes through sources.track with an empty name as null", () => {
		expect(
			buildPayload({ kind: "path", path: "/Users/me/Photos" }, "", {}),
		).toEqual({
			type: "sources.track",
			input: { path: "/Users/me/Photos", name: null, overrides: {} },
		});
	});
});

describe("existing inclusion", () => {
	const sources = [
		source("/Volumes/Archive"),
		source("/Volumes/Archive/Photos/2024"),
		source("/Users/me/Documents"),
		source("/Volumes/Archive/Photos", { device_id: "other-device" }),
	];

	test("finds the exact, enclosing and nested sources on this device", () => {
		const inclusion = inclusionOf("/Volumes/Archive/Photos", sources);
		expect(inclusion.exact).toBeUndefined();
		expect(inclusion.enclosing?.root).toBe("/Volumes/Archive");
		expect(inclusion.nested.map((s) => s.root)).toEqual([
			"/Volumes/Archive/Photos/2024",
		]);
	});

	test("an exact match is reported as such", () => {
		expect(inclusionOf("/Volumes/Archive", sources).exact?.root).toBe(
			"/Volumes/Archive",
		);
	});

	test("a sibling prefix is not containment", () => {
		expect(inclusionOf("/Volumes/Archive2", sources).enclosing).toBeUndefined();
	});
});

describe("containing volume", () => {
	test("picks the longest mounted mount point", () => {
		const volumes = [
			volume({ id: "root", mount_point: "/", mount_type: "System" }),
			volume({ id: "archive" }),
			volume({ id: "gone", mount_point: "/Volumes/Archive/Photos", is_mounted: false }),
		];
		expect(containingVolume("/Volumes/Archive/Photos/x", volumes)?.id).toBe("archive");
		expect(containingVolume("/Users/me", volumes)?.id).toBe("root");
	});

	test("ranks by the mount entry that matched, not the primary one", () => {
		const volumes = [
			volume({ id: "data", mount_point: "/", mount_points: ["/mnt/data"] }),
			volume({ id: "mnt", mount_point: "/mnt" }),
		];
		expect(containingVolume("/mnt/data/file", volumes)?.id).toBe("data");
	});
});

describe("addRefusal", () => {
	const mounted = volume({ id: "pool", name: "pool", mount_point: "/mnt/pool" });
	const locked = volume({
		id: "vault",
		name: "vault",
		mount_point: "/mnt/pool/vault",
		is_mounted: false,
		locked: true,
	});
	const unmounted = volume({
		id: "archive",
		name: "archive",
		display_name: "Archive",
		mount_point: "/mnt/archive",
		is_mounted: false,
	});

	test("a mounted drive or a folder on one adds", () => {
		expect(addRefusal({ kind: "volume", volume: mounted }, [mounted])).toBeUndefined();
		expect(
			addRefusal({ kind: "path", path: "/mnt/pool/photos" }, [mounted, locked]),
		).toBeUndefined();
	});

	test("a locked drive is refused with its key as the reason", () => {
		expect(addRefusal({ kind: "volume", volume: locked }, [mounted, locked])).toBe(
			"vault is locked: its encryption key is not loaded. Load the key and mount it, then add it.",
		);
	});

	test("an unmounted drive is refused under its display name", () => {
		expect(addRefusal({ kind: "volume", volume: unmounted }, [unmounted])).toBe(
			"Archive is not mounted. Mount it, then add it.",
		);
	});

	test("a path at or under an away drive's mount point is refused like the drive", () => {
		const volumes = [mounted, locked, unmounted];
		expect(addRefusal({ kind: "path", path: "/mnt/pool/vault" }, volumes)).toContain(
			"vault is locked",
		);
		expect(
			addRefusal({ kind: "path", path: "/mnt/pool/vault/photos" }, volumes),
		).toContain("vault is locked");
		expect(addRefusal({ kind: "path", path: "/mnt/archive" }, volumes)).toContain(
			"Archive is not mounted",
		);
		expect(addRefusal({ kind: "path", path: "/mnt/archives" }, volumes)).toBeUndefined();
	});
});
