import { useQueryClient } from '@tanstack/react-query';
import React from 'react';
import { Text, View } from 'react-native';
import {
	auth,
	toggleFeatureFlag,
	useBridgeMutation,
	useBridgeQuery,
	useDebugState,
	useFeatureFlags
} from '@sd/client';
import Card from '~/components/layout/Card';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const DebugScreen = ({ navigation }: SettingsStackScreenProps<'Debug'>) => {
	const debugState = useDebugState();
	const featureFlags = useFeatureFlags();
	const origin = useBridgeQuery(['cloud.getApiOrigin']);
	const setOrigin = useBridgeMutation(['cloud.setApiOrigin']);

	const queryClient = useQueryClient();

	return (
		<View style={tw`flex-1 p-4`}>
			<Card style={tw`gap-y-4 bg-app-box`}>
				<Text style={tw`font-semibold text-ink`}>Debug</Text>
				<Button onPress={() => (debugState.rspcLogger = !debugState.rspcLogger)}>
					<Text style={tw`text-ink`}>Toggle rspc logger</Text>
				</Button>
				<Text style={tw`text-ink`}>{JSON.stringify(featureFlags)}</Text>
				<Text style={tw`text-ink`}>{JSON.stringify(debugState)}</Text>
				<Button
					onPress={() => {
						navigation.popToTop();
						navigation.replace('Settings');
						debugState.enabled = false;
					}}
				>
					<Text style={tw`text-ink`}>Disable Debug Mode</Text>
				</Button>
				<Button
					onPress={() => {
						const url =
							origin.data === 'https://app.spacedrive.com'
								? 'http://localhost:3000'
								: 'https://app.spacedrive.com';
						setOrigin.mutateAsync(url).then(async () => {
							await auth.logout();
							await queryClient.invalidateQueries();
						});
					}}
				>
					<Text style={tw`text-ink`}>Toggle API Route ({origin.data})</Text>
				</Button>
				<Button
					onPress={() => {
						navigation.popToTop();
						navigation.navigate('BackfillWaitingStack', {
							screen: 'BackfillWaiting'
						});
					}}
				>
					<Text style={tw`text-ink`}>Go to Backfill Waiting Page</Text>
				</Button>
				<Button
					onPress={async () => {
						await auth.logout();
					}}
				>
					<Text style={tw`text-ink`}>Logout</Text>
				</Button>
			</Card>
		</View>
	);
};

export default DebugScreen;
