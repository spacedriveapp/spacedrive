import { getIcon } from '@sd/assets/util';
import { type PropsWithChildren, useEffect, useLayoutEffect, useMemo, useState } from 'react';
import { Image, View } from 'react-native';
import { DocumentDirectoryPath } from 'react-native-fs';
import {
	type ExplorerItem,
	getExplorerItemData,
	getItemFilePath,
	getItemLocation,
	isDarkTheme
} from '@sd/client';
import { flattenThumbnailKey, useExplorerStore } from '~/stores/explorerStore';
import { tw } from '../../lib/tailwind';

export const getThumbnailUrlByThumbKey = (thumbKey: string[]) =>
	`${DocumentDirectoryPath}/thumbnails/${thumbKey
		.map((i) => encodeURIComponent(i))
		.join('/')}.webp`;

const FileThumbWrapper = ({ children, size = 1 }: PropsWithChildren<{ size: number }>) => (
	<View style={[tw`items-center justify-center`, { width: 80 * size, height: 80 * size }]}>
		{children}
	</View>
);

function useExplorerItemData(explorerItem: ExplorerItem) {
	const explorerStore = useExplorerStore();

	const newThumbnail = !!(
		explorerItem.thumbnail_key &&
		explorerStore.newThumbnails.has(flattenThumbnailKey(explorerItem.thumbnail_key))
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
	const filePath = getItemFilePath(props.data);

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
