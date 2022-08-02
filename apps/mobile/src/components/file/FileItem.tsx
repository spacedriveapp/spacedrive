import { FilePath } from '@sd/core';
import React from 'react';
import { Text, View } from 'react-native';

type FileItemProps = {
	file?: FilePath | null;
};

const FileItem = ({ file }: FileItemProps) => {
	return (
		<View>
			<Text>FileItem</Text>
		</View>
	);
};

export default FileItem;
