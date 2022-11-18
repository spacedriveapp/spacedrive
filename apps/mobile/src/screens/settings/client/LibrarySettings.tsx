import { LibraryConfigWrapped, useBridgeQuery } from '@sd/client';
import { Pen, Trash } from 'phosphor-react-native';
import React from 'react';
import { FlatList, Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import { AnimatedButton } from '~/components/primitive/Button';
import DeleteLibraryDialog from '~/containers/dialog/DeleteLibraryDialog';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

// https://docs.swmansion.com/react-native-gesture-handler/docs/api/components/swipeable/
// ^ Might look better?

function LibraryItem({
	library,
	index,
	navigation
}: {
	library: LibraryConfigWrapped;
	index: number;
	navigation: SettingsStackScreenProps<'LibrarySettings'>['navigation'];
}) {
	return (
		<Card style={tw.style(index !== 0 && 'mt-2')}>
			<View style={tw`flex flex-row items-center justify-between`}>
				<View>
					<Text style={tw`font-semibold text-ink`}>{library.config.name}</Text>
					<Text style={tw`mt-0.5 text-xs text-ink-dull`}>{library.uuid}</Text>
				</View>
				<View style={tw`flex flex-row`}>
					<AnimatedButton size="sm" onPress={() => navigation.replace('LibraryGeneralSettings')}>
						<Pen size={18} color={tw.color('ink')} />
					</AnimatedButton>
					<DeleteLibraryDialog libraryUuid={library.uuid}>
						<AnimatedButton size="sm" style={tw`ml-1.5`}>
							<Trash size={18} color={tw.color('ink')} />
						</AnimatedButton>
					</DeleteLibraryDialog>
				</View>
			</View>
		</Card>
	);
}

const LibrarySettingsScreen = ({ navigation }: SettingsStackScreenProps<'LibrarySettings'>) => {
	const { data: libraries } = useBridgeQuery(['library.list']);

	return (
		<View style={tw`py-4 px-3 flex-1`}>
			<FlatList
				data={libraries}
				keyExtractor={(item) => item.uuid}
				renderItem={({ item, index }) => (
					<LibraryItem navigation={navigation} library={item} index={index} />
				)}
			/>
		</View>
	);
};

export default LibrarySettingsScreen;
