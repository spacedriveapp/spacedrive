import { useDebugState, useFeatureFlags } from '@sd/client';
import React from 'react';
import { Text } from 'react-native';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const DebugScreen = ({ navigation }: SettingsStackScreenProps<'Debug'>) => {
	const debugState = useDebugState();
	const featureFlags = useFeatureFlags();

	return (
		<ScreenContainer style={tw`px-6`} header={{ title: 'Debug', navBack: true }}>
			<Card style={tw`gap-y-4`}>
				<Text style={tw`font-semibold text-ink`}>Debug</Text>
				<Button variant="darkgray" onPress={() => (debugState.rspcLogger = !debugState.rspcLogger)}>
					<Text style={tw`text-ink`}>Toggle rspc logger</Text>
				</Button>
				<Text style={tw`text-ink`}>{JSON.stringify(featureFlags)}</Text>
				<Text style={tw`text-ink`}>{JSON.stringify(debugState)}</Text>
				<Button
					variant="darkgray"
					onPress={() => {
						navigation.popToTop();
						navigation.replace('Settings');
						debugState.enabled = false;
					}}
				>
					<Text style={tw`text-ink`}>Disable Debug Mode</Text>
				</Button>
			</Card>
		</ScreenContainer>
	);
};

export default DebugScreen;
