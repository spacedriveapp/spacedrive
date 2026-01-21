import * as icons from "../icons";
import { LayeredIcons } from "../svgs/ext";
import beardedIconsMapping from "../svgs/ext/icons.json";

// Note: beardedIconUrls is not exported from here for React Native compatibility
// Web/Desktop should import directly from: @sd/assets/svgs/ext/Extras/urls

// Define a type for icon names. This filters out any names with underscores in them.
// The use of 'never' is to make sure that icon types with underscores are not included.
export type IconTypes<K = keyof typeof icons> = K extends `${string}_${string}`
	? never
	: K;

// Create a record of icon names that don't contain underscores.
export const iconNames = Object.fromEntries(
	Object.keys(icons)
		.filter((key) => !key.includes("_")) // Filter out any keys with underscores
		.map((key) => [key, key]), // Map key to [key, key] format
) as Record<IconTypes, string>;

export type IconName = keyof typeof iconNames;

export const getIconByName = (name: IconTypes, isDark?: boolean) => {
	if (!isDark) name = (name + "_Light") as IconTypes;
	return icons[name];
};

/**
 * Gets the appropriate icon based on the given criteria.
 *
 * @param kind - The type of the document.
 * @param isDark - If true, returns the dark mode version of the icon.
 * @param extension - The file extension (if any).
 * @param isDir - If true, the request is for a directory/folder icon.
 */
export const getIcon = (
	kind: string,
	isDark?: boolean,
	extension?: string | null,
	isDir?: boolean,
) => {
	// If the request is for a directory/folder, return the appropriate version.
	if (isDir) return icons[isDark ? "Folder" : "Folder_Light"];

	// Default document icon.
	let document: Extract<keyof typeof icons, "Document" | "Document_Light"> =
		"Document";

	// Modify the extension based on kind and theme (dark/light).
	if (extension) extension = `${kind}_${extension.toLowerCase()}`;
	if (!isDark) {
		document = "Document_Light";
		if (extension) extension += "_Light";
	}

	const lightKind = kind + "_Light";

	// Select the icon based on the given parameters.
	return icons[
		// 1. Check if the specific extension icon exists.
		(extension && extension in icons
			? extension
			: // 2. If in light mode, check if the specific kind in light exists.
				!isDark && lightKind in icons
				? lightKind
				: // 3. Check if a general kind icon exists.
					kind in icons
					? kind
					: // 4. Default to the document (or document light) icon.
						document) as keyof typeof icons
	];
};

export const getLayeredIcon = (kind: string, extension?: string | null) => {
	const iconKind =
		LayeredIcons[
			// Check if specific kind exists.
			kind && kind in LayeredIcons ? kind : "Extras"
		];
	return extension
		? iconKind?.[extension] || LayeredIcons["Extras"]?.[extension]
		: null;
};

/**
 * Gets a bearded icon (file extension badge) name for the given extension.
 * Returns the icon name that can be used to construct the SVG path.
 *
 * @param extension - The file extension (without the dot)
 * @param fileName - Optional full filename for specific file name mappings
 */
export const getBeardedIcon = (
	extension?: string | null,
	fileName?: string | null,
): string | null => {
	if (!extension && !fileName) return null;

	const mapping = beardedIconsMapping as {
		fileExtensions: Record<string, string>;
		fileNames: Record<string, string>;
	};

	// Try filename match first (e.g., "package.json" -> "npm")
	if (fileName && mapping.fileNames[fileName.toLowerCase()]) {
		return mapping.fileNames[fileName.toLowerCase()];
	}
	// Then try extension match (e.g., "ts" -> "typescript")
	else if (extension) {
		const ext = extension.toLowerCase().replace(/^\./, ""); // Remove leading dot if present
		return mapping.fileExtensions[ext] || null;
	}

	return null;
};

/**
 * Gets the 20px variant of an icon if available.
 * These are smaller icons optimized for compact UI elements like path bars.
 *
 * @param kind - The type of the icon (e.g., 'Folder', 'Document', 'Image')
 * @param isDir - If true, returns the Folder20 icon
 */
export const getIcon20 = (kind: string, isDir?: boolean): string | null => {
	if (isDir) {
		return icons["Folder20" as keyof typeof icons] || null;
	}

	const icon20Key = `${kind}20` as keyof typeof icons;
	return icons[icon20Key] || null;
};
