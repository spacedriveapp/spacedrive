import React from 'react';
import { Text, View } from 'react-native';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const ExtensionsSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'ExtensionsSettings'>) => {
	return (
		<ScreenContainer style={tw`px-6`}>
			<Text style={tw`text-ink`}>TODO</Text>
		</ScreenContainer>
	);
};

export default ExtensionsSettingsScreen;
