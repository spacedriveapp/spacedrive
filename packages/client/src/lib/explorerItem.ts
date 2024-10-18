import type { ExplorerItem, ThumbKey } from '../core';
import { getItemFilePath, getItemLocation, getItemObject } from '../utils';
import { humanizeSize } from './humanizeSize';
import { ObjectKind, ObjectKindKey } from './objectKind';

// ItemData is a single data structure understood by the Explorer, we map all ExplorerItems to this structure in this file
// we use `null` instead of `?` optional values intentionally
export interface ItemData {
	name: string | null;
	fullName: string | null;
	size: ReturnType<typeof humanizeSize>;
	kind: ObjectKindKey;
	isDir: boolean;
	casId: string | null;
	extension: string | null;
	locationId: number | null;
	dateIndexed: string | null;
	dateCreated: string | null;
	dateModified: string | null;
	dateAccessed: string | null;
	dateTaken: string | null;
	thumbnailKey: ThumbKey | null; // default behavior is to render a single thumbnail
	thumbnailKeys?: ThumbKey[]; // if set, we can render multiple thumbnails
	hasLocalThumbnail: boolean; // this is overwritten when new thumbnails are generated
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
			else if (data.type === 'NonIndexedPath')
				itemData.kind = ObjectKind[data.item.kind] ?? 'Unknown';

			if (object && 'exif_data' in object && object.exif_data?.media_date) {
				const byteArray = object.exif_data.media_date;
				const dateString = String.fromCharCode.apply(null, byteArray);
				const [date, time] = dateString.replace(/"/g, '').split(' ');
				if (date && time) itemData.dateTaken = `${date}T${time}Z`;
			}

			// Objects only have dateCreated and dateAccessed
			itemData.dateCreated = object?.date_created ?? null;
			itemData.dateAccessed = object?.date_accessed ?? null;
			// handle thumbnail based on provided key
			// This could be better, but for now we're mapping the backend property to two different local properties (thumbnailKey, thumbnailKeys) for backward compatibility
			if (data.thumbnail) {
				itemData.thumbnailKey = data.thumbnail;
				itemData.thumbnailKeys = [data.thumbnail];
			}

			itemData.hasLocalThumbnail = data.has_created_thumbnail;
			// handle file path
			const filePath = getItemFilePath(data);
			if (filePath) {
				itemData.name = filePath.name;
				itemData.fullName = getFullName(filePath.name, filePath.extension);
				itemData.size = humanizeSize(filePath.size_in_bytes_bytes);
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
					itemData.size = humanizeSize(
						location.total_capacity - location.available_capacity
					);

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
			itemData.customIcon = 'Tag';
			itemData.thumbnailKey = data.thumbnails[0] ?? null;
			itemData.thumbnailKeys = data.thumbnails;
			itemData.hasLocalThumbnail = !!data.thumbnails;
			itemData.kind = 'Label';
			break;
		}
	}

	return itemData;
}

export function getFullName(
	filePathName: string | null,
	filePathExtension?: string | null
): string {
	return `${filePathName}${filePathExtension ? `.${filePathExtension}` : ''}`;
}

function getDefaultItemData(kind: ObjectKindKey = 'Unknown'): ItemData {
	return {
		name: null,
		fullName: null,
		size: humanizeSize(0),
		kind: 'Unknown',
		isDir: false,
		casId: null,
		extension: null,
		locationId: null,
		dateIndexed: null,
		dateCreated: null,
		dateModified: null,
		dateAccessed: null,
		dateTaken: null,
		thumbnailKey: null,
		hasLocalThumbnail: false,
		customIcon: null
	};
}
