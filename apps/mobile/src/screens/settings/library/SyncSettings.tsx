import { inferSubscriptionResult } from '@oscartbeaumont-sd/rspc-client';
import { Circle } from 'phosphor-react-native';
import React, { useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import {
	Procedures,
	useDiscoveredPeers,
	useFeatureFlag,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription
} from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const ACTORS = {
	Ingest: 'Sync Ingest',
	CloudSend: 'Cloud Sync Sender',
	CloudReceive: 'Cloud Sync Receiver',
	CloudIngest: 'Cloud Sync Ingest'
};

const SyncSettingsScreen = ({ navigation }: SettingsStackScreenProps<'SyncSettings'>) => {
	const syncEnabled = useLibraryQuery(['sync.enabled']);

	const backfillSync = useLibraryMutation(['sync.backfill'], {
		onSuccess: async () => {
			await syncEnabled.refetch();
		}
	});

	const [data, setData] = useState<inferSubscriptionResult<Procedures, 'library.actors'>>({});
	const [startBackfill, setStart] = useState(false);

	useLibrarySubscription(['library.actors'], { onData: setData });

	useEffect(() => {
		if (startBackfill === true) {
			console.log('Starting Backfill!');

			navigation.navigate('BackfillWaitingStack', {
				screen: 'BackfillWaiting'
			});

			// Force re-render?
		}
	}, [startBackfill, navigation]);

	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6`}>
			{syncEnabled.data === false ? (
				<Button
					variant={'accent'}
					onPress={() => {
						setStart(true);
					}}
				>
					<Text>Start Backfill Operations</Text>
				</Button>
			) : (
				<View>
					<Text
						style={tw`flex flex-col items-center justify-center text-left text-white`}
					>
						Ingester
						<OnlineIndicator online={data[ACTORS.Ingest] ?? false} />
					</Text>
					<View>
						{data[ACTORS.Ingest] ? (
							<StopButton name={ACTORS.Ingest} />
						) : (
							<StartButton name={ACTORS.Ingest} />
						)}
					</View>
					<Text
						style={tw`flex flex-col items-center justify-center text-left text-white`}
					>
						Sender
						<OnlineIndicator online={data[ACTORS.CloudSend] ?? false} />
					</Text>
					<View>
						{data[ACTORS.CloudSend] ? (
							<StopButton name={ACTORS.CloudSend} />
						) : (
							<StartButton name={ACTORS.CloudSend} />
						)}
					</View>
					<Text
						style={tw`flex flex-col items-center justify-center text-left text-white`}
					>
						Receiver
						<OnlineIndicator online={data[ACTORS.CloudReceive] ?? false} />
					</Text>
					<View>
						{data[ACTORS.CloudReceive] ? (
							<StopButton name={ACTORS.CloudReceive} />
						) : (
							<StartButton name={ACTORS.CloudReceive} />
						)}
					</View>
					<Text
						style={tw`flex flex-col items-center justify-center text-left text-white`}
					>
						Cloud Ingester
						<OnlineIndicator online={data[ACTORS.CloudIngest] ?? false} />
					</Text>
					<View>
						{data[ACTORS.CloudIngest] ? (
							<StopButton name={ACTORS.CloudIngest} />
						) : (
							<StartButton name={ACTORS.CloudIngest} />
						)}
					</View>
				</View>
			)}
		</ScreenContainer>
	);
};

export default SyncSettingsScreen;

function OnlineIndicator({ online }: { online: boolean }) {
	const size = 10;
	return online ? (
		<Circle size={size} color="#00ff0a" weight="fill" />
	) : (
		<Circle size={size} color="#ff0600" weight="fill" />
	);
}

function StartButton({ name }: { name: string }) {
	const startActor = useLibraryMutation(['library.startActor']);

	return (
		<Button
			variant="accent"
			disabled={startActor.isLoading}
			onPress={() => startActor.mutate(name)}
		>
			{startActor.isLoading ? <Text>Starting</Text> : <Text>Start</Text>}
		</Button>
	);
}

function StopButton({ name }: { name: string }) {
	const stopActor = useLibraryMutation(['library.stopActor']);

	return (
		<Button
			variant="accent"
			disabled={stopActor.isLoading}
			onPress={() => stopActor.mutate(name)}
		>
			{stopActor.isLoading ? <Text>Stopping</Text> : <Text>Stop</Text>}
		</Button>
	);
}
