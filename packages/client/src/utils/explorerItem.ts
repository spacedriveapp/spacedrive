import { getItemFilePath, getItemLocation, getItemObject, type ObjectKindKey } from '..';
import type { ExplorerItem } from '../core';
import { byteSize } from '../lib';
import { ObjectKind } from './objectKind';

// ItemData is a single data structure understood by the Explorer, we map all ExplorerItems to this structure in this file
// we use `null` instead of `?` optional values intentionally
export interface ItemData {
	name: string | null;
	fullName: string | null;
	size: ReturnType<typeof byteSize>;
	kind: ObjectKindKey;
	isDir: boolean;
	casId: string | null;
	extension: string | null;
	locationId: number | null;
	dateIndexed: string | null;
	dateCreated: string | null;
	dateModified: string | null;
	dateAccessed: string | null;
	thumbnailKey: string[];
	// this is overwritten when new thumbnails are generated
	hasLocalThumbnail: boolean;
	customIcon: string | null;
}

// this function maps an ExplorerItem to an ItemData
export function getExplorerItemData(data?: ExplorerItem | null): ItemData {
	const itemData = getDefaultItemData();
	if (!data) return itemData;

	// a typesafe switch statement for all the different types of ExplorerItems
	switch (data.type) {
		// the getItemObject and getItemFilePath type-guards mean we can handle the following types in one case
		case 'Object':
		case 'NonIndexedPath':
		case 'Path': {
			// handle object
			const object = getItemObject(data);

			if (object?.kind) itemData.kind = ObjectKind[object?.kind] ?? 'Unknown';
			// Objects only have dateCreated and dateAccessed
			itemData.dateCreated = object?.date_created ?? null;
			itemData.dateAccessed = object?.date_accessed ?? null;
			// handle thumbnail based on provided key
			itemData.thumbnailKey = data.thumbnail_key ?? [];
			itemData.hasLocalThumbnail = data.has_local_thumbnail ?? false;
			// handle file path
			const filePath = getItemFilePath(data);
			if (filePath) {
				itemData.name = filePath.name;
				itemData.fullName = `${filePath.name}${
					filePath.extension ? `.${filePath.extension}` : ''
				}`;
				itemData.size = byteSize(filePath.size_in_bytes_bytes);
				itemData.isDir = filePath.is_dir ?? false;
				itemData.extension = filePath.extension?.toLocaleLowerCase() ?? null;
				//
				if ('cas_id' in filePath) itemData.casId = filePath.cas_id;
				if ('location_id' in filePath) itemData.locationId = filePath.location_id;
				if ('date_indexed' in filePath) itemData.dateIndexed = filePath.date_indexed;
				if ('date_modified' in filePath) itemData.dateModified = filePath.date_modified;
			}
			break;
		}
		// the following types do not have a file_path or an object associated, and must be handled from scratch
		case 'Location': {
			const location = getItemLocation(data);
			if (location) {
				if (location.total_capacity != null && location.available_capacity != null)
					itemData.size = byteSize(location.total_capacity - location.available_capacity);

				itemData.name = location.name;
				itemData.fullName = location.name;
				itemData.kind = ObjectKind[ObjectKind.Folder] ?? 'Unknown';
				itemData.isDir = true;
				itemData.locationId = location.id;
			}
			break;
		}
		case 'SpacedropPeer': {
			itemData.name = data.item.name;
			itemData.customIcon = 'Laptop';
			break;
		}
		case 'Label': {
			itemData.name = data.item.name;
			break;
		}
	}

	return itemData;
}

function getDefaultItemData(kind: ObjectKindKey = 'Unknown'): ItemData {
	return {
		name: null,
		fullName: null,
		size: byteSize(0),
		kind: 'Unknown',
		isDir: false,
		casId: null,
		extension: null,
		locationId: null,
		dateIndexed: null,
		dateCreated: null,
		dateModified: null,
		dateAccessed: null,
		thumbnailKey: [],
		hasLocalThumbnail: false,
		customIcon: null
	};
}
