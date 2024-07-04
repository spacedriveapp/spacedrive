import { useNavigation } from '@react-navigation/native';
import { Plus } from 'phosphor-react-native';
import { useRef, useState } from 'react';
import { FlatList, Text, View } from 'react-native';
import { useLibraryQuery } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

import Empty from '../layout/Empty';
import Fade from '../layout/Fade';
import { LocationItem } from '../locations/LocationItem';
import ImportModal from '../modal/ImportModal';
import { Button } from '../primitive/Button';

const BrowseLocations = () => {
	const navigation = useNavigation<
		BrowseStackScreenProps<'Browse'>['navigation'] &
			SettingsStackScreenProps<'Settings'>['navigation']
	>();

	const modalRef = useRef<ModalRef>(null);
	const [showAll, setShowAll] = useState(false);
	const result = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const locations = result.data;

	return (
		<View style={tw`gap-5`}>
			<View style={tw`flex-row items-center justify-between px-5`}>
				<Text style={tw`text-lg font-bold text-white`}>Locations</Text>
				<View style={tw`flex-row gap-3`}>
					<Button
						style={twStyle(`rounded-full`, {
							borderColor: showAll
								? tw.color('accent')
								: tw.color('border-app-lightborder')
						})}
						variant="outline"
						onPress={() => setShowAll((prev) => !prev)}
					>
						<Text style={tw`text-xs text-ink`}>
							{showAll ? 'Show less' : 'Show all'} ({locations?.length})
						</Text>
					</Button>
					<Button
						onPress={() => modalRef.current?.present()}
						style={tw`flex-row gap-1 rounded-full`}
						variant="gray"
					>
						<Plus size={10} weight="bold" style={tw`text-white`} />
						<Text style={tw`text-xs text-ink`}>Add</Text>
					</Button>
				</View>
			</View>
			<View style={tw`relative -m-1`}>
				<Fade color="black" width={30} height="100%">
					<FlatList
						data={locations}
						ListEmptyComponent={
							<Empty description="You have not added any locations" icon="Folder" />
						}
						numColumns={showAll ? 3 : 1}
						horizontal={showAll ? false : true}
						contentContainerStyle={twStyle(locations?.length === 0 && 'w-full', 'px-5')}
						key={showAll ? '_locations' : 'alllocationcols'}
						keyExtractor={(item) => item.id.toString()}
						scrollEnabled={showAll ? false : true}
						showsHorizontalScrollIndicator={false}
						renderItem={({ item }) => {
							return (
								<LocationItem
									location={item}
									style={twStyle(showAll && 'max-w-[31%] flex-1')}
									editLocation={() =>
										navigation.navigate('SettingsStack', {
											screen: 'EditLocationSettings',
											params: { id: item.id },
											initial: false
										})
									}
									onPress={() => navigation.navigate('Location', { id: item.id })}
								/>
							);
						}}
					/>
				</Fade>
			</View>
			<ImportModal ref={modalRef} />
		</View>
	);
};

export default BrowseLocations;
