import React, { useRef, useState } from 'react';
import { FlatList, NativeScrollEvent, Pressable, View, ViewStyle } from 'react-native';
import {
	ExplorerItem,
	getExplorerItemData,
	getItemFilePath,
	getItemObject,
	isPath,
	useLibraryQuery
} from '@sd/client';
import Fade from '~/components/layout/Fade';
import { ModalRef } from '~/components/layout/Modal';
import AddTagModal from '~/components/modal/AddTagModal';
import { InfoPill, PlaceholderPill } from '~/components/primitive/InfoPill';
import { tw, twStyle } from '~/lib/tailwind';

type Props = {
	data: ExplorerItem;
	style?: ViewStyle;
	contentContainerStyle?: ViewStyle;
	columnCount?: number;
};

const InfoTagPills = ({ data, style, contentContainerStyle, columnCount = 3 }: Props) => {
	const objectData = getItemObject(data);
	const filePath = getItemFilePath(data);
	const [startedScrolling, setStartedScrolling] = useState(false);
	const [reachedBottom, setReachedBottom] = useState(true); // needs to be set to true for initial rendering fade to be correct

	const tagsQuery = useLibraryQuery(['tags.getForObject', objectData?.id ?? -1], {
		enabled: objectData != null
	});

	const ref = useRef<ModalRef>(null);
	const tags = tagsQuery.data;
	const isDir = data && isPath(data) ? data.item.is_dir : false;

	// Fade the tag pills when scrolling
	const fadeScroll = ({ layoutMeasurement, contentOffset, contentSize }: NativeScrollEvent) => {
		const isScrolling = contentOffset.y > 0;
		setStartedScrolling(isScrolling);

		const hasReachedBottom = layoutMeasurement.height + contentOffset.y >= contentSize.height;
		setReachedBottom(hasReachedBottom);
	};

	return (
		<>
			<View style={twStyle('mb-3 mt-2 flex-col flex-wrap items-start gap-1', style)}>
				<View style={tw`flex-row gap-1`}>
					<Pressable style={tw`relative z-10`} onPress={() => ref.current?.present()}>
						<PlaceholderPill text={'Tags'} />
					</Pressable>
					{/* Kind */}
					<InfoPill text={isDir ? 'Folder' : getExplorerItemData(data).kind} />
					{/* Extension */}
					{filePath?.extension && <InfoPill text={filePath.extension} />}
				</View>
				<View
					onLayout={(e) => {
						if (e.nativeEvent.layout.height >= 80) {
							setReachedBottom(false);
						} else {
							setReachedBottom(true);
						}
					}}
					style={twStyle(`relative flex-row flex-wrap gap-1 overflow-hidden`)}
				>
					<Fade
						fadeSides="top-bottom"
						orientation="vertical"
						color="bg-app-modal"
						width={20}
						topFadeStyle={twStyle(startedScrolling ? 'mt-0' : 'h-0')}
						bottomFadeStyle={twStyle(reachedBottom ? 'h-0' : 'h-6')}
						height="100%"
					>
						<FlatList
							onScroll={(e) => fadeScroll(e.nativeEvent)}
							style={tw`max-h-20 w-full grow-0`}
							data={tags}
							scrollEventThrottle={1}
							showsVerticalScrollIndicator={false}
							numColumns={columnCount}
							contentContainerStyle={twStyle(`gap-1`, contentContainerStyle)}
							columnWrapperStyle={
								tags && twStyle(tags.length > 0 && `flex-wrap gap-1`)
							}
							key={tags?.length}
							keyExtractor={(item) =>
								item.id.toString() + Math.floor(Math.random() * 10)
							}
							renderItem={({ item }) => (
								<InfoPill
									text={item.name ?? 'Unnamed Tag'}
									containerStyle={twStyle({ backgroundColor: item.color + 'CC' })}
									textStyle={tw`text-white`}
								/>
							)}
						/>
					</Fade>
				</View>
			</View>
			<AddTagModal ref={ref} />
		</>
	);
};

export default InfoTagPills;
