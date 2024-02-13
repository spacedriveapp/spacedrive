import { useNavigation } from '@react-navigation/native';
import React, { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { Location } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import Fade from '../layout/Fade';
import { ModalRef } from '../layout/Modal';
import ImportModal from '../modal/ImportModal';
import { Button } from '../primitive/Button';
import NewCard from './NewCard';
import OverviewSection from './OverviewSection';
import StatCard from './StatCard';

interface Props {
	locations?: Location[];
}

const Locations = ({ locations }: Props) => {
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();
	const modalRef = useRef<ModalRef>(null);

	return (
		<View>
			<OverviewSection title="Locations" count={locations?.length}>
				<View style={tw`flex-row items-center`}>
					<Fade height={'100%'} width={30} color="mobile-screen">
						<FlatList
							horizontal
							data={locations}
							contentContainerStyle={tw`px-7`}
							showsHorizontalScrollIndicator={false}
							keyExtractor={(location) => location.id.toString()}
							ItemSeparatorComponent={() => <View style={tw`w-2`} />}
							ListFooterComponent={() => (
								<NewCard
									style={twStyle(locations?.length !== 0 ? 'ml-2' : 'ml-0')}
									icons={['HDD', 'Folder', 'Globe', 'SD']}
									text="Connect a local path, volume or network location to Spacedrive."
									button={() => (
										<Button
											style={tw`mt-2.5`}
											variant="outline"
											onPress={() => {
												modalRef.current?.present();
											}}
										>
											<Text style={tw`font-bold text-ink`}>Add Location</Text>
										</Button>
									)}
								/>
							)}
							showsVerticalScrollIndicator={false}
							renderItem={({ item }) => (
								<Pressable
									onPress={() =>
										navigation.navigate('BrowseStack', {
											screen: 'Location',
											params: { id: item.id }
										})
									}
								>
									<StatCard
										connectionType={null}
										totalSpace={item.size_in_bytes || [0]}
										name={item.name || ''}
										color="#0362FF"
										icon="Folder"
									/>
								</Pressable>
							)}
						/>
					</Fade>
				</View>
			</OverviewSection>
			<ImportModal ref={modalRef} />
		</View>
	);
};

export default Locations;
