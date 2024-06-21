import { DocumentDirectoryPath } from '@dr.pogodin/react-native-fs';
import { getIcon } from '@sd/assets/util';
import {
	ThumbKey,
	getExplorerItemData,
	getItemLocation,
	isDarkTheme,
	type ExplorerItem
} from '@sd/client';
import { Image } from 'expo-image';
import { useEffect, useLayoutEffect, useMemo, useState, type PropsWithChildren } from 'react';
import { View } from 'react-native';
import { flattenThumbnailKey, useExplorerStore } from '~/stores/explorerStore';

import { tw } from '../../lib/tailwind';

// NOTE: `file://` is required for Android to load local files!
export const getThumbnailUrlByThumbKey = (thumbKey: ThumbKey) => {
	return `file://${DocumentDirectoryPath}/thumbnails/${encodeURIComponent(
		thumbKey.base_directory_str
	)}/${encodeURIComponent(thumbKey.shard_hex)}/${encodeURIComponent(thumbKey.cas_id)}.webp`;
};

const FileThumbWrapper = ({ children, size = 1 }: PropsWithChildren<{ size: number }>) => (
	<View style={[tw`items-center justify-center`, { width: 80 * size, height: 80 * size }]}>
		{children}
	</View>
);

function useExplorerItemData(explorerItem: ExplorerItem) {
	const explorerStore = useExplorerStore();

	const firstThumbnail =
		explorerItem.type === 'Label'
			? explorerItem.thumbnails?.[0]
			: 'thumbnail' in explorerItem && explorerItem.thumbnail;

	const newThumbnail = !!(
		firstThumbnail && explorerStore.newThumbnails.has(flattenThumbnailKey(firstThumbnail))
	);

	return useMemo(() => {
		const itemData = getExplorerItemData(explorerItem);

		if (!itemData.hasLocalThumbnail) {
			itemData.hasLocalThumbnail = newThumbnail;
		}

		return itemData;
	}, [explorerItem, newThumbnail]);
}

enum ThumbType {
	Icon,
	// Original,
	Thumbnail,
	Location
}

type FileThumbProps = {
	data: ExplorerItem;
	/**
	 * This is multiplier for calculating icon size
	 * default: `1`
	 */
	size?: number;
	// loadOriginal?: boolean;
};

export default function FileThumb({ size = 1, ...props }: FileThumbProps) {
	const itemData = useExplorerItemData(props.data);
	const locationData = getItemLocation(props.data);

	const [src, setSrc] = useState<null | string>(null);
	const [thumbType, setThumbType] = useState(ThumbType.Icon);
	// const [loaded, setLoaded] = useState<boolean>(false);

	useLayoutEffect(() => {
		// Reset src when item changes, to allow detection of yet not updated src
		setSrc(null);
		// setLoaded(false);

		if (locationData) {
			setThumbType(ThumbType.Location);
			// } else if (props.loadOriginal) {
			// 	setThumbType(ThumbType.Original);
		} else if (itemData.hasLocalThumbnail) {
			setThumbType(ThumbType.Thumbnail);
		} else {
			setThumbType(ThumbType.Icon);
		}
	}, [locationData, itemData]);

	// This sets the src to the thumbnail url
	useEffect(() => {
		const { casId, kind, isDir, extension, locationId, thumbnailKey } = itemData;

		// ???
		// const locationId =
		// 	itemLocationId ?? (parent?.type === 'Location' ? parent.location.id : null);

		switch (thumbType) {
			// case ThumbType.Original:
			// 	if (locationId) {
			// 		setSrc(
			// 			platform.getFileUrl(
			// 				library.uuid,
			// 				locationId,
			// 				filePath?.id || props.data.item.id,
			// 				// Workaround Linux webview not supporting playing video and audio through custom protocol urls
			// 				kind == 'Video' || kind == 'Audio'
			// 			)
			// 		);
			// 	} else {
			// 		setThumbType(ThumbType.Thumbnail);
			// 	}
			// 	break;
			case ThumbType.Thumbnail:
				if (casId && thumbnailKey) {
					setSrc(getThumbnailUrlByThumbKey(thumbnailKey));
				} else {
					setThumbType(ThumbType.Icon);
				}
				break;
			case ThumbType.Location:
				setSrc(getIcon('Folder', isDarkTheme(), extension, true));
				break;
			default:
				if (isDir !== null) setSrc(getIcon(kind, isDarkTheme(), extension, isDir));
				break;
		}
	}, [itemData, thumbType]);
	return (
		<FileThumbWrapper size={size}>
			{(() => {
				if (src == null) return null;
				let source = null;
				// getIcon returns number for some magic reason
				if (typeof src === 'number') {
					source = src;
				} else {
					source = { uri: src };
				}
				return <Image source={source} style={{ width: 70 * size, height: 70 * size }} />;
			})()}
		</FileThumbWrapper>
	);
}
