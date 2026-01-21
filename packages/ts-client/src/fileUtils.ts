import type { ContentKind, File } from "./generated/types";

/**
 * Get the content kind for a file, preferring content_identity.kind if available,
 * falling back to content_kind (identified by extension during ephemeral indexing).
 */
export function getContentKind(file: File | null | undefined): ContentKind {
	return file?.content_identity?.kind ?? file?.content_kind ?? "unknown";
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
