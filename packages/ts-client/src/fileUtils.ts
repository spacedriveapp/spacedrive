import type { ContentKind, File } from "./generated/types";

/**
 * Get the content kind for a file, preferring content_identity.kind if available,
 * falling back to content_kind (identified by extension during indexing).
 */
export function getContentKind(file: File | null | undefined): ContentKind {
	return file?.content_identity?.kind ?? file?.content_kind ?? "unknown";
}

/**
 * The most specific kind name a file carries: the extension kind id
 * (`com.spacedrive.photos:raw`) the content identity phase stored, whether
 * or not that extension is loaded now, else the built-in kind. The inspector
 * and the file kinds page label with this; previews and icons resolve it
 * against the loaded extensions and fall back to `getContentKind`.
 */
export function getContentKindName(file: File | null | undefined): string {
	return file?.content_kind_name ?? getContentKind(file);
}

/**
 * Get the appropriate kind string for icon resolution.
 * This transforms the content kind into a capitalized string suitable for icon lookup.
 */
export function getFileKindForIcon(file: File | null | undefined): string {
	const contentKind = getContentKind(file);
	const fileKind =
		contentKind && contentKind !== "unknown"
			? contentKind
			: file?.kind === "File"
				? file.extension || "File"
				: file?.kind || "File";
	return fileKind.charAt(0).toUpperCase() + fileKind.slice(1);
}
