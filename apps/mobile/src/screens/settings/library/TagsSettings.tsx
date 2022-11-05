import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const TagsSettingsScreen = ({ navigation }: SettingsStackScreenProps<'TagsSettings'>) => {
	return (
		<View>
			<Text>TagsSettingsScreen</Text>
		</View>
	);
};

export default TagsSettingsScreen;
