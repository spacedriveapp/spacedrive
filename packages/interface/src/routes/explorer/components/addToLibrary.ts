import type {
	AddDefaults,
	AddOverrides,
	SourceConfig,
	SourceInfo,
	StorePlacement,
	TrackSourceInput,
	Volume,
	VolumeTrackInput,
} from "@sd/ts-client";

/**
 * The state behind the Add to Library modal, kept free of React so the
 * defaults -> overrides -> payload path can be tested on its own.
 *
 * The library's defaults come from Library Settings > Adding content. The
 * modal resolves them for the selected scope, lets the user change each one,
 * and sends only what differs as the add's overrides, so a one-off change
 * never writes back to the defaults unless the user asks for that.
 */

export type AddTarget =
	| { kind: "path"; path: string }
	| { kind: "volume"; volume: Volume };

/** Every add setting resolved to a value. */
export interface AddSettings {
	placement: StorePlacement;
	keep_offline_copy: boolean;
	unfiltered: boolean;
	identify_content: boolean;
}

export const ADD_SETTING_KEYS = [
	"placement",
	"keep_offline_copy",
	"unfiltered",
	"identify_content",
] as const satisfies readonly (keyof AddSettings)[];

/** The core's own defaults, used while the library's have not loaded. */
export const BUILT_IN_DEFAULTS: AddSettings = {
	placement: "in_library",
	keep_offline_copy: true,
	unfiltered: false,
	identify_content: true,
};

/** Fill the library's saved defaults with the built-in value for each field it leaves unset. */
export function libraryDefaults(adding: AddDefaults | undefined): AddSettings {
	return {
		placement: adding?.placement ?? BUILT_IN_DEFAULTS.placement,
		keep_offline_copy:
			adding?.keep_offline_copy ?? BUILT_IN_DEFAULTS.keep_offline_copy,
		unfiltered: adding?.unfiltered ?? BUILT_IN_DEFAULTS.unfiltered,
		identify_content:
			adding?.identify_content ?? BUILT_IN_DEFAULTS.identify_content,
	};
}

/** The path the add registers: a volume target adds its mount point. */
export function targetPath(target: AddTarget): string {
	return target.kind === "path" ? target.path : target.volume.mount_point;
}

/** The last component of a path, for a name the user can keep or edit. */
export function suggestedName(target: AddTarget): string {
	if (target.kind === "volume") return target.volume.name;
	const parts = target.path.split("/").filter(Boolean);
	return parts[parts.length - 1] ?? target.path;
}

function isUnder(path: string, root: string): boolean {
	const trimmed = root.endsWith("/") && root.length > 1 ? root.slice(0, -1) : root;
	return path === trimmed || path.startsWith(trimmed === "/" ? "/" : `${trimmed}/`);
}

/** The mounted volume whose mount point is the longest prefix of the path. */
export function containingVolume(
	path: string,
	volumes: readonly Volume[],
): Volume | undefined {
	let best: { volume: Volume; depth: number } | undefined;
	for (const volume of volumes) {
		if (!volume.is_mounted) continue;
		for (const mount of [volume.mount_point, ...volume.mount_points]) {
			if (!mount || !isUnder(path, mount)) continue;
			if (!best || mount.length > best.depth) best = { volume, depth: mount.length };
		}
	}
	return best?.volume;
}

/**
 * Why the add cannot start at all, when the target is a drive Spacedrive
 * knows but cannot read: a whole drive that is unmounted or locked, or a
 * path at or under such a drive's mount point. The core refuses these
 * (`sources.track` names the volume's state), and the directory left at
 * the mount point would otherwise be added as an empty folder of the
 * parent drive.
 */
export function addRefusal(
	target: AddTarget,
	volumes: readonly Volume[],
): string | undefined {
	const away =
		target.kind === "volume"
			? target.volume
			: volumes.find(
					(volume) =>
						!volume.is_mounted &&
						[volume.mount_point, ...volume.mount_points].some(
							(mount) => mount && isUnder(target.path, mount),
						),
				);
	if (!away || away.is_mounted) return undefined;
	const name = away.display_name || away.name;
	return away.locked
		? `${name} is locked: its encryption key is not loaded. Load the key and mount it, then add it.`
		: `${name} is not mounted. Mount it, then add it.`;
}

/** How the library already covers the path, before anything is added. */
export interface Inclusion {
	/** A source whose root is exactly this path. */
	exact?: SourceInfo;
	/** The innermost source whose root contains this path. */
	enclosing?: SourceInfo;
	/** Sources rooted below this path. */
	nested: SourceInfo[];
}

/**
 * Only this device's filesystem sources count: a replica from another
 * device names a path on that device, not here.
 */
export function inclusionOf(
	path: string,
	sources: readonly SourceInfo[],
): Inclusion {
	const local = sources.filter(
		(source): source is SourceInfo & { root: string } =>
			!source.device_id && typeof source.root === "string",
	);
	const exact = local.find((source) => source.root === path);
	const enclosing = local
		.filter((source) => source.root !== path && isUnder(path, source.root))
		.sort((a, b) => b.root.length - a.root.length)[0];
	const nested = local.filter(
		(source) => source.root !== path && isUnder(source.root, path),
	);
	return { exact, enclosing, nested };
}

/** Why a placement cannot be chosen for this add, when it cannot. */
export interface PlacementChoice {
	available: boolean;
	reason?: string;
}

/**
 * Mirrors the core's refusals so the modal explains them before the add
 * instead of after it fails.
 */
export function placementChoices(
	volume: Volume | undefined,
	existing: SourceInfo | undefined,
): Record<StorePlacement, PlacementChoice> {
	if (existing) {
		const reason =
			"This source already has a catalog. Moving it is a later step in source settings.";
		return {
			in_library: { available: existing.placement !== "on_source", reason },
			on_source: { available: existing.placement === "on_source", reason },
		};
	}
	const in_library: PlacementChoice = { available: true };
	if (!volume) {
		return {
			in_library,
			on_source: {
				available: false,
				reason: "Spacedrive has not matched this path to a drive, so it cannot place a catalog on it.",
			},
		};
	}
	if (volume.mount_type === "Network" || volume.cloud_identifier) {
		return {
			in_library,
			on_source: {
				available: false,
				reason: "A network or cloud volume cannot hold its own catalog; the catalog stays in the library on this device.",
			},
		};
	}
	if (volume.is_read_only) {
		return {
			in_library,
			on_source: {
				available: false,
				reason: "This drive is read-only, so nothing can be written beside its files.",
			},
		};
	}
	return { in_library, on_source: { available: true } };
}

/**
 * The settings the add starts from: the library's defaults, adjusted the
 * way the core adjusts them for this scope.
 *
 * An external drive added whole captures everything unless the add says
 * otherwise, which `volumes.track` applies on its own; showing it here keeps
 * the preview honest. A re-add of an existing source starts from what that
 * source saved, since the core keeps those values unless the add names them.
 * A placement the drive cannot hold falls back to the library.
 */
export function effectiveDefaults(
	library: AddSettings,
	target: AddTarget,
	volume: Volume | undefined,
	existing: SourceInfo | undefined,
): AddSettings {
	if (existing?.settings) {
		return settingsFromConfig(existing.settings);
	}
	const choices = placementChoices(volume, existing);
	const placement = choices[library.placement].available
		? library.placement
		: "in_library";
	const unfiltered =
		target.kind === "volume" && target.volume.mount_type === "External"
			? true
			: library.unfiltered;
	return { ...library, placement, unfiltered };
}

export function settingsFromConfig(config: SourceConfig): AddSettings {
	return {
		placement: config.placement ?? BUILT_IN_DEFAULTS.placement,
		keep_offline_copy:
			config.keep_offline_copy ?? BUILT_IN_DEFAULTS.keep_offline_copy,
		unfiltered: config.unfiltered ?? BUILT_IN_DEFAULTS.unfiltered,
		identify_content:
			config.identify_content ?? BUILT_IN_DEFAULTS.identify_content,
	};
}

/** The fields the user changed from what the modal showed, and nothing else. */
export function overridesFrom(
	defaults: AddSettings,
	chosen: AddSettings,
): AddOverrides {
	const overrides: AddOverrides = {};
	for (const key of ADD_SETTING_KEYS) {
		if (chosen[key] !== defaults[key]) {
			Object.assign(overrides, { [key]: chosen[key] });
		}
	}
	return overrides;
}

export function changedKeys(
	defaults: AddSettings,
	chosen: AddSettings,
): (keyof AddSettings)[] {
	return ADD_SETTING_KEYS.filter((key) => chosen[key] !== defaults[key]);
}

export type AddPayload =
	| { type: "sources.track"; input: TrackSourceInput }
	| { type: "volumes.track"; input: VolumeTrackInput };

/**
 * One add, whichever entry point chose the scope. A whole drive goes
 * through `volumes.track` so the volume row takes the display name and the
 * core's external-drive capture rule applies; a folder goes through
 * `sources.track`, which tracks the containing volume on its own.
 */
export function buildPayload(
	target: AddTarget,
	name: string,
	overrides: AddOverrides,
): AddPayload {
	const trimmed = name.trim();
	if (target.kind === "volume") {
		return {
			type: "volumes.track",
			input: {
				fingerprint: target.volume.fingerprint,
				display_name: trimmed || null,
				overrides,
			},
		};
	}
	return {
		type: "sources.track",
		input: { path: target.path, name: trimmed || null, overrides },
	};
}

