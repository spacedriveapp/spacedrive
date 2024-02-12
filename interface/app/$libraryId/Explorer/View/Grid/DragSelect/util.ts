export const SELECTABLE_DATA_ATTRIBUTE = 'data-selectable';
export const SELECTABLE_ID_DATA_ATTRIBUTE = 'data-selectable-id';
export const SELECTABLE_INDEX_DATA_ATTRIBUTE = 'data-selectable-index';

export function getElementById(id: string) {
	return document.querySelector(`[${SELECTABLE_ID_DATA_ATTRIBUTE}="${id}"]`);
}

export function getElementByIndex(index: number) {
	return document.querySelector(`[${SELECTABLE_INDEX_DATA_ATTRIBUTE}="${index}"]`);
}

export function getElementIndex(element: Element) {
	const index = element.getAttribute(SELECTABLE_INDEX_DATA_ATTRIBUTE);
	return index ? Number(index) : null;
}
