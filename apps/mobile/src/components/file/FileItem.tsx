import { FilePath } from '@sd/core';
import React from 'react';
import { Text, View } from 'react-native';
import Svg, { Path } from 'react-native-svg';

import icons from '../../assets/icons/file';
import tw from '../../lib/tailwind';
import FolderIcon from '../icons/Folder';

type FileItemProps = {
	file?: FilePath | null;
};

// TODO: Menu for file actions (File details, Share etc.)
// TODO: Sheet Modal for file details

const FileItem = ({ file }: FileItemProps) => {
	return (
		<View style={tw`w-[100px] h-[100px]`}>
			<View style={tw`items-center`}>
				{/* Folder Icons/Thumbnail etc. */}
				<View style={tw`h-[60px]`}>
					{file?.is_dir ? (
						<FolderIcon size={48} />
					) : file?.file?.has_thumbnail ? (
						<>{/* TODO */}</>
					) : (
						<View style={tw`w-[48px] h-[60px] mt-1.5 m-auto relative`}>
							<Svg
								style={tw`absolute top-0 left-0`}
								fill={tw.color('gray-550')}
								width={43}
								height={56}
								viewBox="0 0 65 81"
							>
								<Path d="M0 8a8 8 0 0 1 8-8h31.686a8 8 0 0 1 5.657 2.343L53.5 10.5l9.157 9.157A8 8 0 0 1 65 25.314V73a8 8 0 0 1-8 8H8a8 8 0 0 1-8-8V8Z" />
							</Svg>
							<Svg
								style={tw`absolute top-[1px] right-1`}
								fill={tw.color('gray-500')}
								width={15}
								height={15}
								viewBox="0 0 41 41"
							>
								<Path d="M41.412 40.558H11.234C5.03 40.558 0 35.528 0 29.324V0l41.412 40.558Z" />
							</Svg>
							{/* File Icon & Extension */}
							<View style={tw`absolute w-full h-full items-center justify-center`}>
								{file?.extension && icons[file.extension as keyof typeof icons] ? (
									(() => {
										const Icon = icons[file.extension as keyof typeof icons];
										return <Icon width={20} height={20} style={tw`mt-2`} />;
									})()
								) : (
									<></>
								)}
								<Text style={tw`mt-1 text-[10px] font-bold text-center uppercase text-gray-450`}>
									{file?.extension}
								</Text>
							</View>
						</View>
					)}
				</View>
				{/* Name */}
				<View style={tw`px-1.5 py-[1px] mt-1`}>
					<Text numberOfLines={1} style={tw`text-gray-300 text-center text-xs font-medium`}>
						{file?.name}
					</Text>
				</View>
			</View>
		</View>
	);
};

export default FileItem;
