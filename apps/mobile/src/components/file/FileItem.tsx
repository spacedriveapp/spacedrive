import { FilePath } from '@sd/core';
import React from 'react';
import { Text, View } from 'react-native';

type FileItemProps = {
	file?: FilePath | null;
};

const FileItem = ({ file }: FileItemProps) => {
	// If DIR show folder icon
	// else if file has thumbnail show thumbnail
	// else show file with ext. icon
	// else show default icon
	return (
		<View>
			<Text>FileItem</Text>
		</View>
	);
};

export default FileItem;
