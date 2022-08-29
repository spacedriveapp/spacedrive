import React from 'react';
import { Text, View } from 'react-native';
import Svg, { Path } from 'react-native-svg';

import icons from '../../assets/icons/file';
import tw from '../../lib/tailwind';
import { FilePath } from '../device/Device';
import FolderIcon from '../icons/FolderIcon';

type FileIconProps = {
	file?: FilePath | null;
	/**
	 * This is multiplier for calculating icon size
	 * default: `1`
	 */
	size?: number;
};

const FileIcon = ({ file, size = 1 }: FileIconProps) => {
	return (
		<View style={[tw`justify-center`, { width: 60 * size, height: 60 * size }]}>
			{file?.is_dir ? (
				<View style={tw`items-center`}>
					<FolderIcon size={50 * size} />
				</View>
			) : file?.file?.has_thumbnail ? (
				<>{/* TODO */}</>
			) : (
				<View style={[tw`m-auto relative`, { width: 45 * size, height: 60 * size }]}>
					<Svg
						style={tw`absolute top-0 left-0`}
						fill={tw.color('gray-550')}
						width={45 * size}
						height={60 * size}
						viewBox="0 0 65 81"
					>
						<Path d="M0 8a8 8 0 0 1 8-8h31.686a8 8 0 0 1 5.657 2.343L53.5 10.5l9.157 9.157A8 8 0 0 1 65 25.314V73a8 8 0 0 1-8 8H8a8 8 0 0 1-8-8V8Z" />
					</Svg>
					<Svg
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
						{file?.extension &&
							icons[file.extension] &&
							(() => {
								const Icon = icons[file.extension];
								return <Icon width={18 * size} height={18 * size} style={tw`mt-2`} />;
							})()}
						<Text
							style={[
								tw`mt-1 font-bold text-center uppercase text-gray-450`,
								{
									fontSize: 10 * (size * 0.8)
								}
							]}
						>
							{file?.extension}
						</Text>
					</View>
				</View>
			)}
		</View>
	);
};

export default FileIcon;
