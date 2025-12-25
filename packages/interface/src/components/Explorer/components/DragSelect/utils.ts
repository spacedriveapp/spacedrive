/**
 * Data attribute used to mark elements as selectable
 */
export const SELECTABLE_DATA_ATTRIBUTE = "data-selectable";

/**
 * Get the index of a selectable element from its data attribute
 */
export function getElementIndex(element: Element): number | null {
	const fileId = element.getAttribute("data-file-id");
	if (!fileId) return null;

	const index = element.getAttribute("data-index");
	return index ? parseInt(index, 10) : null;
}

/**
 * Get the file ID from a selectable element
 */
export function getElementFileId(element: Element): string | null {
	return element.getAttribute("data-file-id");
}

/**
 * Check if element is marked as selectable
 */
export function isSelectable(element: Element): boolean {
	return element.hasAttribute(SELECTABLE_DATA_ATTRIBUTE);
}

/**
 * Detect if running on Windows
 */
export function isWindows(): boolean {
	return navigator.platform.toLowerCase().includes("win");
}
