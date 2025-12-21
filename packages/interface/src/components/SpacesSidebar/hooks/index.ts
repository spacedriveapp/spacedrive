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
	type DropTargetType,
} from "./spaceItemUtils";

// Space item hooks
export { useSpaceItemActive } from "./useSpaceItemActive";
export { useSpaceItemDropZones, type UseSpaceItemDropZonesResult } from "./useSpaceItemDropZones";
export { useSpaceItemContextMenu } from "./useSpaceItemContextMenu";

// Space data hooks
export { useSpaces, useSpaceLayout } from "./useSpaces";

