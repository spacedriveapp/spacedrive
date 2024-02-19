import { useNavigation } from '@react-navigation/native';
import { Plus } from 'phosphor-react-native';
import { useRef } from 'react';
import { Pressable, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import { TagItem } from '~/components/browse/BrowseTags';
import Fade from '~/components/layout/Fade';
import { ModalRef } from '~/components/layout/Modal';
import ScreenContainer from '~/components/layout/ScreenContainer';
import CreateTagModal from '~/components/modal/tag/CreateTagModal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

interface Props {
	viewStyle?: 'grid' | 'list';
}

export default function Tags({ viewStyle = 'list' }: Props) {
	const tags = useLibraryQuery(['tags.list']);
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();
	const modalRef = useRef<ModalRef>(null);

	useNodes(tags.data?.nodes);
	const tagData = useCache(tags.data?.items);
	return (
		<ScreenContainer scrollview={false} style={tw`relative px-7 py-0`}>
			<Pressable
				style={tw`absolute bottom-7 right-7 z-10 flex h-12 w-12 items-center justify-center rounded-full bg-accent`}
				onPress={() => {
					modalRef.current?.present();
				}}
			>
				<Plus size={20} weight="bold" style={tw`text-ink`} />
			</Pressable>
			<Fade
				fadeSides="top-bottom"
				orientation="vertical"
				color="mobile-screen"
				width={30}
				height="100%"
			>
				<FlatList
					data={tagData}
					renderItem={({ item }) => (
						<TagItem
							tagStyle={twStyle(viewStyle === 'grid' ? 'w-[105px]' : 'w-full')}
							viewStyle={viewStyle}
							tag={item}
							onPress={() => {
								navigation.navigate('BrowseStack', {
									screen: 'Tag',
									params: { id: item.id, color: item.color! }
								});
							}}
						/>
					)}
					numColumns={viewStyle === 'grid' ? 3 : 1}
					columnWrapperStyle={viewStyle === 'grid' && tw`justify-between`}
					horizontal={false}
					keyExtractor={(item) => item.id.toString()}
					showsHorizontalScrollIndicator={false}
					ItemSeparatorComponent={() => <View style={tw`h-2.5`} />}
					contentContainerStyle={tw`py-5`}
				/>
			</Fade>
			<CreateTagModal ref={modalRef} />
		</ScreenContainer>
	);
}
