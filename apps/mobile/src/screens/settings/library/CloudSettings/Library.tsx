import { CloudLibrary, useLibraryContext, useLibraryMutation } from '@sd/client';
import { CheckCircle, XCircle } from 'phosphor-react-native';
import { useMemo } from 'react';
import { Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import { Button } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { tw } from '~/lib/tailwind';
import { logout, useAuthStateSnapshot } from '~/stores/auth';

import { InfoBox } from './CloudSettings';

interface LibraryProps {
	cloudLibrary?: CloudLibrary;
}

const Library = ({ cloudLibrary }: LibraryProps) => {
	const authState = useAuthStateSnapshot();
	const { library } = useLibraryContext();
	const syncLibrary = useLibraryMutation(['cloud.library.sync']);
	const thisInstance = useMemo(
		() => cloudLibrary?.instances.find((instance) => instance.uuid === library.instance_id),
		[cloudLibrary, library.instance_id]
	);

	return (
		<Card style={tw`w-full`}>
			<View style={tw`flex-row items-center justify-between`}>
				<Text style={tw`font-medium text-ink`}>Library</Text>
				{authState.status === 'loggedIn' && (
					<Button variant="gray" size="sm" onPress={logout}>
						<Text style={tw`text-xs font-semibold text-ink`}>Logout</Text>
					</Button>
				)}
			</View>
			<Divider style={tw`mb-4 mt-2`} />
			<SettingsTitle style={tw`mb-2`}>Name</SettingsTitle>
			<InfoBox>
				<Text style={tw`text-ink`}>{cloudLibrary?.name}</Text>
			</InfoBox>
			<Button
				disabled={syncLibrary.isLoading || thisInstance !== undefined}
				variant="gray"
				onPress={() => syncLibrary.mutate(null)}
				style={tw`mt-2 flex-row gap-1 py-2`}
			>
				{thisInstance ? (
					<CheckCircle size={16} weight="fill" color={tw.color('green-400')} />
				) : (
					<XCircle
						style={tw`rounded-full`}
						size={16}
						weight="fill"
						color={tw.color('red-500')}
					/>
				)}
				<Text style={tw`text-sm font-semibold text-ink`}>
					{thisInstance !== undefined ? 'Library synced' : 'Library not synced'}
				</Text>
			</Button>
		</Card>
	);
};

export default Library;
