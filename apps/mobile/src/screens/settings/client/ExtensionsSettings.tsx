import { ArrowLeft } from 'phosphor-react-native';
import React, { useEffect } from 'react';
import { Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const ExtensionsSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'ExtensionsSettings'>) => {
	useEffect(() => {
		navigation.setOptions({
			headerBackImage: () => (
				<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
			)
		});
	});

	return (
		<View>
			<Text style={tw`text-ink`}>TODO</Text>
		</View>
	);
};

export default ExtensionsSettingsScreen;
