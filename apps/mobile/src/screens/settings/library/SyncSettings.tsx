import { inferSubscriptionResult } from '@oscartbeaumont-sd/rspc-client';
import {
	Procedures,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription
} from '@sd/client';
import { MotiView } from 'moti';
import { Circle } from 'phosphor-react-native';
import React, { useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const SyncSettingsScreen = ({ navigation }: SettingsStackScreenProps<'SyncSettings'>) => {
	const syncEnabled = useLibraryQuery(['sync.enabled']);
	const [data, setData] = useState<inferSubscriptionResult<Procedures, 'library.actors'>>({});

	const [startBackfill, setStart] = useState(false);

	useLibrarySubscription(['library.actors'], { onData: (data) => {
		setData(data);
	} });

	useEffect(() => {
		if (startBackfill === true) {
			navigation.navigate('BackfillWaitingStack', {
				screen: 'BackfillWaiting'
			});
		}
	}, [startBackfill, navigation]);

	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6`}>
			{syncEnabled.data === false ? (
				<Button
					variant={'accent'}
					onPress={() => setStart(true)}
				>
					<Text>Start Backfill Operations</Text>
				</Button>
			) : (
				<View style={tw`flex-row flex-wrap gap-2`}>
					{Object.keys(data).map((key) => {
						return (
							<Card style={tw`w-[48%]`} key={key}>
							<OnlineIndicator online={data[key] ?? false} />
							<Text
								key={key}
								style={tw`flex-col items-center justify-center mt-1 mb-3 text-left text-white`}
							>
								{key}
							</Text>
								{data[key] ? (
									<StopButton name={key} />
								) : (
									<StartButton name={key} />
								)}
							</Card>
						)
						})}
				</View>
					)}
				</ScreenContainer>
	);
}

export default SyncSettingsScreen;

function OnlineIndicator({ online }: { online: boolean }) {
	const size = 8;
	return (
	<View style={tw`items-center justify-center w-6 h-6 p-2 mb-1 border rounded-full border-app-inputborder bg-app-input`}>
	{online ? (
		<View style={tw`relative items-center justify-center`}>
			<MotiView
				from={{ scale: 0 }}
				animate={{ scale: 1.5, opacity: 0 }}
				transition={{ type: 'timing', duration: 500, loop: true, delay: 500}}
			 style={tw`absolute z-10 items-center justify-center w-2 h-2 bg-red-500 rounded-full`} />
			<View style={tw`w-2 h-2 bg-green-500 rounded-full`} />
		</View>
	) : (
		<Circle size={size} color={tw.color('red-400')} weight="fill" />
	)}
	</View>
	)
}

function StartButton({ name }: { name: string }) {
	const startActor = useLibraryMutation(['library.startActor']);
	return (
		<Button
			variant="accent"
			size="sm"
			disabled={startActor.isLoading}
			onPress={() => startActor.mutate(name)}
		>
			<Text style={tw`text-xs font-medium text-ink`}>
				{startActor.isLoading ? 'Starting' : 'Start'}
			</Text>
			{startActor.isLoading ? <Text>Starting</Text> : <Text>Start</Text>}
		</Button>
	);
}

function StopButton({ name }: { name: string }) {
	const stopActor = useLibraryMutation(['library.stopActor']);
	return (
		<Button
			variant="accent"
			size="sm"
			disabled={stopActor.isLoading}
			onPress={() => stopActor.mutate(name)}
		>
			<Text style={tw`text-xs font-medium text-ink`}>
				{stopActor.isLoading ? 'Stopping' : 'Stop'}
			</Text>
		</Button>
	);
}
