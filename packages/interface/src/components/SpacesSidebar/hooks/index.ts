// Space item utilities
export {
  buildDropTargetPath,
  type DropTargetType,
  getDropTargetType,
  type IconData,
  type ItemMetadata,
  isDropTargetItem,
  isFavoritesItem,
  isFileKindsItem,
  isLocationItem,
  isOverviewItem,
  isPathItem,
  isRawLocation,
  isRecentsItem,
  isTagItem,
  isVolumeItem,
  type ResolveMetadataOptions,
  resolveItemMetadata,
} from "./spaceItemUtils";

// Space item hooks
export { useSpaceItemActive } from "./useSpaceItemActive";
export { useSpaceItemContextMenu } from "./useSpaceItemContextMenu";
export {
  type UseSpaceItemDropZonesResult,
  useSpaceItemDropZones,
} from "./useSpaceItemDropZones";

// Space data hooks
export { useSpaceLayout, useSpaces } from "./useSpaces";
