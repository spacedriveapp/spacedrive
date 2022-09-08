import { useNavigation } from '@react-navigation/native';
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { useSnapshot } from 'valtio';

import tw from '../../lib/tailwind';
import { SharedScreenProps } from '../../navigation/SharedScreens';
import { fileModalStore } from '../../stores/modalStore';
import { FilePath } from '../../types/bindings';
import FileIcon from './FileIcon';

type FileItemProps = {
	file?: FilePath | null;
};

// TODO: Menu for file actions (File details, Share etc.)

const FileItem = ({ file }: FileItemProps) => {
	const { fileRef, setData } = useSnapshot(fileModalStore);

	const navigation = useNavigation<SharedScreenProps<'Location'>['navigation']>();

	function handlePress() {
		if (!file) return;

		if (file.is_dir) {
			navigation.navigate('Location', { id: file.location_id });
		} else {
			setData(file);
			fileRef.current.present();
		}
	}

	return (
		<Pressable onPress={handlePress}>
			<View style={tw`w-[90px] h-[80px] items-center`}>
				{/* Folder Icons/Thumbnail etc. */}
				<FileIcon file={file} />
				<View style={tw`px-1.5 py-[1px] mt-1`}>
					<Text numberOfLines={1} style={tw`text-gray-300 text-center text-xs font-medium`}>
						{file?.name}
					</Text>
				</View>
			</View>
		</Pressable>
	);
};

export default FileItem;
