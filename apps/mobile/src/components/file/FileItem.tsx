import { FilePath } from '@sd/core';
import React from 'react';
import { Text, View } from 'react-native';

import tw from '../../lib/tailwind';
import FolderIcon from '../icons/Folder';

type FileItemProps = {
	file?: FilePath | null;
};

// TODO: Menu for file actions (File details, Share etc.)
// TODO: Sheet Modal for file details

const FileItem = ({ file }: FileItemProps) => {
	return (
		<View style={tw`w-24 h-24`}>
			<View style={tw`items-center`}>
				{/* Folder Icons/Thumbnail etc. */}
				<>
					{file?.is_dir ? (
						<FolderIcon size={48} />
					) : file?.file?.has_thumbnail ? (
						<>{/* TODO */}</>
					) : (
						<View>
							<Text>wat</Text>
						</View>
					)}
				</>
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
