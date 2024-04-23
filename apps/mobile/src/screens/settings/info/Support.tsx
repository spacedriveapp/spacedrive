import React from 'react';
import { Text } from 'react-native';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';

const SupportScreen = () => {
	return (
		<ScreenContainer
		style={tw`justify-start px-6 py-5`}
		header={{
			title: 'Support',
			navBack: true,
		}}>
			<Text style={tw`text-ink`}>TODO</Text>
		</ScreenContainer>
	);
};

export default SupportScreen;
