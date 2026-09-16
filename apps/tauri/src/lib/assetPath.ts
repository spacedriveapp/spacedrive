const MACOS_DATA_VOLUME_PREFIX = '/System/Volumes/Data';

const MACOS_ASSET_ALIASES = [
	`${MACOS_DATA_VOLUME_PREFIX}/Users/`,
	`${MACOS_DATA_VOLUME_PREFIX}/Volumes/`
];

/** Converts macOS APFS data-volume paths to the aliases used by Tauri's asset scope. */
export function normalizeAssetProtocolPath(filePath: string): string {
	if (MACOS_ASSET_ALIASES.some((prefix) => filePath.startsWith(prefix))) {
		return filePath.slice(MACOS_DATA_VOLUME_PREFIX.length);
	}

	return filePath;
}
