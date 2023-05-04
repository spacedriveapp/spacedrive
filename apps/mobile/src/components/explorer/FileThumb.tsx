import * as icons from '@sd/assets/icons';
import { PropsWithChildren } from 'react';
import { Image, View } from 'react-native';
import { DocumentDirectoryPath } from 'react-native-fs';
import { ExplorerItem, ObjectKind, isObject, isPath } from '@sd/client';
import { tw } from '../../lib/tailwind';
import FolderIcon from '../icons/FolderIcon';

type FileThumbProps = {
	data: ExplorerItem;
	/**
	 * This is multiplier for calculating icon size
	 * default: `1`
	 */
	size?: number;
};

export const getThumbnailUrlById = (casId: string) =>
	`${DocumentDirectoryPath}/thumbnails/${encodeURIComponent(casId)}.webp`;

type KindType = keyof typeof icons | 'Unknown';

function getExplorerItemData(data: ExplorerItem) {
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;

	const filePath = isObject(data) ? data.item.file_paths[0] : data.item;

	return {
		casId: filePath?.cas_id || null,
		isDir: isPath(data) && data.item.is_dir,
		kind: ObjectKind[objectData?.kind || 0] as KindType,
		hasThumbnail: data.has_thumbnail,
		extension: filePath?.extension
	};
}

const FileThumbWrapper = ({ children, size = 1 }: PropsWithChildren<{ size: number }>) => (
	<View style={[tw`items-center justify-center`, { width: 80 * size, height: 80 * size }]}>
		{children}
	</View>
);

export default function FileThumb({ data, size = 1 }: FileThumbProps) {
	const { casId, isDir, kind, hasThumbnail, extension } = getExplorerItemData(data);

	if (isPath(data) && data.item.is_dir) {
		return (
			<FileThumbWrapper size={size}>
				<FolderIcon size={70 * size} />
			</FileThumbWrapper>
		);
	}

	if (hasThumbnail && casId) {
		// TODO: Handle Image checkers bg?
		return (
			<FileThumbWrapper size={size}>
				<Image
					source={{ uri: getThumbnailUrlById(casId) }}
					resizeMode="contain"
					style={tw`h-full w-full`}
				/>
			</FileThumbWrapper>
		);
	}

	// Default icon
	let icon = icons['Document'];

	if (isDir) {
		icon = icons['Folder'];
	} else if (
		kind &&
		extension &&
		icons[`${kind}_${extension.toLowerCase()}` as keyof typeof icons]
	) {
		// e.g. Document_pdf
		icon = icons[`${kind}_${extension.toLowerCase()}` as keyof typeof icons];
	} else if (kind !== 'Unknown' && kind && icons[kind]) {
		icon = icons[kind];
	}

	// TODO: Handle video thumbnails (do we have ffmpeg on mobile?)

	// // 10 percent of the size
	// const videoBarsHeight = Math.floor(size / 10);

	// // calculate 16:9 ratio for height from size
	// const videoHeight = Math.floor((size * 9) / 16) + videoBarsHeight * 2;

	return (
		<FileThumbWrapper size={size}>
			<Image source={icon} style={{ width: 70 * size, height: 70 * size }} />
		</FileThumbWrapper>
	);
}
