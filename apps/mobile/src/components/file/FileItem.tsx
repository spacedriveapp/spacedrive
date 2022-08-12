import React from 'react';
import { Text, View } from 'react-native';
import Svg, { Path } from 'react-native-svg';

import icons from '../../assets/icons/file';
import tw from '../../lib/tailwind';
import { FilePath } from '../device/Device';
import FolderIcon from '../icons/Folder';

type FileItemProps = {
	file?: FilePath | null;
};

// TODO: Menu for file actions (File details, Share etc.)
// TODO: Sheet Modal for file details

const FileItem = ({ file }: FileItemProps) => {
	return (
		<View style={tw`w-[90px] h-[100px]`}>
			<View style={tw`items-center`}>
				{/* Folder Icons/Thumbnail etc. */}
				<View style={tw`w-[60px] h-[60px] justify-center`}>
					{file?.is_dir ? (
						<View style={tw`items-center`}>
							<FolderIcon size={54} />
						</View>
					) : file?.file?.has_thumbnail ? (
						<>{/* TODO */}</>
					) : (
						<View style={tw`w-[45px] h-[60px] m-auto relative`}>
							<Svg
								style={tw`absolute top-0 left-0`}
								fill={tw.color('gray-550')}
								width={45}
								height={60}
								viewBox="0 0 65 81"
							>
								<Path d="M0 8a8 8 0 0 1 8-8h31.686a8 8 0 0 1 5.657 2.343L53.5 10.5l9.157 9.157A8 8 0 0 1 65 25.314V73a8 8 0 0 1-8 8H8a8 8 0 0 1-8-8V8Z" />
							</Svg>
							<Svg
								style={tw`absolute top-[2px] -right-[0.6px]`}
								fill={tw.color('gray-500')}
								width={15}
								height={15}
								viewBox="0 0 41 41"
							>
								<Path d="M41.412 40.558H11.234C5.03 40.558 0 35.528 0 29.324V0l41.412 40.558Z" />
							</Svg>
							{/* File Icon & Extension */}
							<View style={tw`absolute w-full h-full items-center justify-center`}>
								{file?.extension && icons[file.extension] ? (
									(() => {
										const Icon = icons[file.extension];
										return <Icon width={18} height={18} style={tw`mt-2`} />;
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
