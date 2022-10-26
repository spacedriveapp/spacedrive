import VideoSvg from '@sd/assets/svgs/video.svg';
import ZipSvg from '@sd/assets/svgs/zip.svg';
import { ExplorerItem, usePlatform } from '@sd/client';
import { Suspense, useMemo } from 'react';
import { Image, Text, View } from 'react-native';
import Svg, { Path } from 'react-native-svg';
import { useExplorerStore } from '~/stores/explorerStore';
import { isObject, isPath } from '~/types/helper';

import icons from '../../assets/icons/file';
import tw from '../../lib/tailwind';
import FolderIcon from '../icons/FolderIcon';

type FileThumbProps = {
	data: ExplorerItem;
	/**
	 * This is multiplier for calculating icon size
	 * default: `1`
	 */
	size?: number;
	kind?: 'video' | 'image' | 'audio' | 'zip' | 'other';
};

export default function FileThumb({ data, size = 1, kind }: FileThumbProps) {
	const explorerStore = useExplorerStore();
	const platform = usePlatform();

	const Icon = useMemo(() => {
		const Icon = icons[data.extension];
		return Icon;
	}, [data.extension]);

	if (isPath(data) && data.is_dir)
		return (
			<View style={[tw`justify-center items-center`, { width: 60 * size, height: 60 * size }]}>
				<FolderIcon size={50 * size} />
			</View>
		);

	const cas_id = isObject(data) ? data.cas_id : data.object?.cas_id;

	if (!cas_id) return undefined;

	const has_thumbnail = isObject(data)
		? data.has_thumbnail
		: isPath(data)
		? data.object?.has_thumbnail
		: !!explorerStore.newThumbnails[cas_id];

	const url = platform.getThumbnailUrlById(cas_id);

	// Thumbnail
	if (has_thumbnail && url) {
		// TODO: Thumbnail job doesn't work?
		// not styled yet
		return <Image source={{ uri: url }} style={tw`w-[50px] h-[50px]`} />;
	}

	// Video
	if (kind === 'video') {
		return (
			<View style={[tw`justify-center items-center`, { width: 60 * size, height: 60 * size }]}>
				<VideoSvg width={50 * size} height={50 * size} />
			</View>
		);
	}

	// Zip
	if (kind === 'zip') {
		return (
			<View style={[tw`justify-center items-center`, { width: 60 * size, height: 60 * size }]}>
				<ZipSvg width={50 * size} height={50 * size} />
			</View>
		);
	}

	// Default file icon
	return (
		<View style={[tw`justify-center`, { width: 60 * size, height: 60 * size }]}>
			<View style={[tw`m-auto relative`, { width: 45 * size, height: 60 * size }]}>
				<Svg
					// Background
					style={tw`absolute top-0 left-0`}
					fill={tw.color('gray-550')}
					width={45 * size}
					height={60 * size}
					viewBox="0 0 65 81"
				>
					<Path d="M0 8a8 8 0 0 1 8-8h31.686a8 8 0 0 1 5.657 2.343L53.5 10.5l9.157 9.157A8 8 0 0 1 65 25.314V73a8 8 0 0 1-8 8H8a8 8 0 0 1-8-8V8Z" />
				</Svg>
				<Svg
					// Peel
					style={tw`absolute top-[2px] -right-[0.6px]`}
					fill={tw.color('gray-500')}
					width={15 * size}
					height={15 * size}
					viewBox="0 0 41 41"
				>
					<Path d="M41.412 40.558H11.234C5.03 40.558 0 35.528 0 29.324V0l41.412 40.558Z" />
				</Svg>
				{/* File Icon & Extension */}
				<View style={tw`absolute w-full h-full items-center justify-center`}>
					{Icon && (
						<Suspense fallback={<></>}>
							<Icon width={18 * size} height={18 * size} style={tw`mt-2`} />
						</Suspense>
					)}
					<Text
						style={[
							tw`mt-1 font-bold text-center uppercase text-gray-450`,
							{
								fontSize: 10 * (size * 0.8)
							}
						]}
					>
						{data.extension}
					</Text>
				</View>
			</View>
		</View>
	);
}
