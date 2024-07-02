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
import { Icon } from '~/components/icons/Icon';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const SyncSettingsScreen = ({ navigation }: SettingsStackScreenProps<'SyncSettings'>) => {
	const syncEnabled = useLibraryQuery(['sync.enabled']);
	const [data, setData] = useState<inferSubscriptionResult<Procedures, 'library.actors'>>({});

	const [startBackfill, setStart] = useState(false);

	useLibrarySubscription(['library.actors'], { onData: setData });

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
				<View style={tw`flex-1 justify-center`}>
					<Card style={tw`relative flex-col items-center gap-5 py-6`}>
					<View style={tw`flex-col items-center gap-2`}>
					<Icon name="Sync" size={64} />
					<Text style={tw`max-w-[70%] text-center leading-5 text-ink`}>
						To enable sync, please start the backfill operations
					</Text>
					</View>
						<Button
							variant={'accent'}
							style={tw`mx-auto max-w-[82%]`}
							onPress={() => setStart(true)}
						>
							<Text style={tw`font-medium text-white`}>
								Start backfill
							</Text>
						</Button>
					</Card>
				</View>
			) : (
				<View style={tw`flex-row flex-wrap gap-2`}>
					{Object.keys(data).map((key) => {
						return (
							<Card style={tw`w-[48%]`} key={key}>
								<OnlineIndicator online={data[key] ?? false} />
								<Text
									key={key}
									style={tw`mb-3 mt-1 flex-col items-center justify-center text-left text-xs text-white`}
								>
									{key}
								</Text>
								{data[key] ? <StopButton name={key} /> : <StartButton name={key} />}
							</Card>
						);
					})}
				</View>
			)}
		</ScreenContainer>
	);
};

export default SyncSettingsScreen;

function OnlineIndicator({ online }: { online: boolean }) {
	const size = 6;
	return (
		<View
			style={tw`mb-1 h-6 w-6 items-center justify-center rounded-full border border-app-inputborder bg-app-input p-2`}
		>
			{online ? (
				<View style={tw`relative items-center justify-center`}>
					<MotiView
						from={{ scale: 0, opacity: 1 }}
						animate={{ scale: 3, opacity: 0 }}
						transition={{
							type: 'timing',
							duration: 1500,
							loop: true,
							repeatReverse: false,
							delay: 1000
						}}
						style={tw`absolute z-10 h-2 w-2 items-center justify-center rounded-full bg-green-500`}
					/>
					<View style={tw`h-2 w-2 rounded-full bg-green-500`} />
				</View>
			) : (
				<Circle size={size} color={tw.color('red-400')} weight="fill" />
			)}
		</View>
	);
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
