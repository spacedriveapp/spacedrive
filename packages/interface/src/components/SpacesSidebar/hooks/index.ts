// Space item utilities
export {
	isOverviewItem,
	isRecentsItem,
	isFavoritesItem,
	isFileKindsItem,
	isLocationItem,
	isVolumeItem,
	isTagItem,
	isPathItem,
	isRawLocation,
	isDropTargetItem,
	getDropTargetType,
	buildDropTargetPath,
	resolveItemMetadata,
	type IconData,
	type ItemMetadata,
	type ResolveMetadataOptions,
	type DropTargetType
} from './spaceItemUtils';

export {useSpaceItemContextMenu} from './useSpaceItemContextMenu';

// Space data hooks
export {useSpaces, useSpaceLayout} from './useSpaces';
