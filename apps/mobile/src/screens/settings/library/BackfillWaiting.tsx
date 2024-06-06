import { useEffect } from 'react';
import { Button, Text } from 'react-native';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const BackfillWaiting = ({ navigation }: SettingsStackScreenProps<'BackfillWaiting'>) => {
	const syncEnabled = useLibraryQuery(['sync.enabled']);

	const enableSync = useLibraryMutation(['sync.backfill'], {});

	useEffect(() => {
		async function _() {
			await enableSync.mutateAsync(null).then(() => navigation.navigate('SyncSettings'));
		}

		_();
	}, []);

	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6`}>
			<Text
				style={tw`flex h-full w-full flex-col items-center justify-center p-5 text-center text-xl text-white`}
			>
				Library is being backfilled right now for Sync! Please hold while this process takes
				place.
			</Text>

			<Button onPress={() => navigation.goBack()} title="Go Back" />
		</ScreenContainer>
	);
};

export default BackfillWaiting;
