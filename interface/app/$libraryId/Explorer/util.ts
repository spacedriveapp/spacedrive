import { type ExplorerItem } from '@sd/client';
import { ExplorerParamsSchema } from '~/app/route-schemas';
import { useZodSearchParams } from '~/hooks';

export function useExplorerSearchParams() {
	return useZodSearchParams(ExplorerParamsSchema);
}

export const pubIdToString = (pub_id: number[]) =>
	pub_id.map((b) => b.toString(16).padStart(2, '0')).join('');

export const uniqueId = (item: ExplorerItem | { pub_id: number[] }) => {
	if ('pub_id' in item) return pubIdToString(item.pub_id);

	const { type } = item;

	switch (type) {
		case 'NonIndexedPath':
			return item.item.path;
		case 'SpacedropPeer':
		case 'Label':
			return item.item.name;
		default:
			return pubIdToString(item.item.pub_id);
	}
};

export function getItemId(index: number, items: ExplorerItem[]) {
	const item = items[index];
	return item ? uniqueId(item) : undefined;
}

export function getItemData(index: number, items: ExplorerItem[]) {
	return items[index];
}
