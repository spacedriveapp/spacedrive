import { CaretLeft, Plus } from 'phosphor-react-native';
import { forwardRef, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { FlatList, NativeScrollEvent, Pressable, Text, View } from 'react-native';
import {
	getItemObject,
	Tag,
	useLibraryMutation,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw, twStyle } from '~/lib/tailwind';
import { useActionsModalStore } from '~/stores/modalStore';

import Card from '../layout/Card';
import Fade from '../layout/Fade';
import { Modal, ModalRef } from '../layout/Modal';
import { Button } from '../primitive/Button';
import CreateTagModal from './tag/CreateTagModal';

const AddTagModal = forwardRef<ModalRef, unknown>((_, ref) => {
	const { data } = useActionsModalStore();

	// Wrapped in memo to ensure that the data is not undefined on initial render
	const objectData = data && getItemObject(data);

	const modalRef = useForwardedRef(ref);
	const newTagRef = useRef<ModalRef>(null);
	const [startedScrolling, setStartedScrolling] = useState(false);
	const [reachedBottom, setReachedBottom] = useState(true); // needs to be set to true for initial rendering fade to be correct

	const rspc = useRspcLibraryContext();
	const tagsQuery = useLibraryQuery(['tags.list']);
	const tagsObjectQuery = useLibraryQuery(['tags.getForObject', objectData?.id ?? -1]);
	const mutation = useLibraryMutation(['tags.assign'], {
		onSuccess: () => {
			// this makes sure that the tags are updated in the UI
			rspc.queryClient.invalidateQueries(['tags.getForObject']);
			rspc.queryClient.invalidateQueries(['search.paths']);
			modalRef.current?.dismiss();
		}
	});

	const tagsData = tagsQuery.data;
	const tagsObject = tagsObjectQuery.data;

	const [selectedTags, setSelectedTags] = useState<
		{
			id: number;
			unassign: boolean;
			selected: boolean;
		}[]
	>([]);

	// get the tags that are already applied to the object
	const appliedTags = useMemo(() => {
		if (!tagsObject) return [];
		return tagsObject?.map((t) => t.id);
	}, [tagsObject]);

	// set selected tags when tagsOfObject.data is available
	useEffect(() => {
		if (!tagsObject) return;
		//we want to set the selectedTags if there are applied tags
		//this deals with an edge case of clearing the tags onDismiss of the Modal
		if (selectedTags.length === 0 && appliedTags.length > 0) {
			setSelectedTags(
				(tagsObject ?? []).map((tag) => ({
					id: tag.id,
					unassign: false,
					selected: true
				}))
			);
		}
	}, [tagsObject, appliedTags, selectedTags]);

	// check if tag is selected
	const isSelected = useCallback(
		(id: number) => {
			const findTag = selectedTags.find((t) => t.id === id);
			return findTag?.selected ?? false;
		},
		[selectedTags]
	);

	const selectTag = useCallback(
		(id: number) => {
			//check if tag is already selected
			const findTag = selectedTags.find((t) => t.id === id);
			if (findTag) {
				//if tag is already selected, update its selected value
				setSelectedTags((prev) =>
					prev.map((t) =>
						t.id === id ? { ...t, selected: !t.selected, unassign: !t.unassign } : t
					)
				);
			} else {
				//if tag is not selected, select it
				setSelectedTags((prev) => [...prev, { id, unassign: false, selected: true }]);
			}
		},
		[selectedTags]
	);

	const assignHandler = async () => {
		const targets =
			data &&
			'id' in data.item &&
			(data.type === 'Object'
				? {
						Object: data.item.id
					}
				: {
						FilePath: data.item.id
					});

		// in order to support assigning multiple tags
		// we need to make multiple mutation calls
		if (targets)
			await Promise.all([
				...selectedTags.map(
					async (tag) =>
						await mutation.mutateAsync({
							targets: [targets],
							tag_id: tag.id,
							unassign: tag.unassign
						})
				)
			]);
	};

	// Fade the tags when scrolling
	const fadeScroll = ({ layoutMeasurement, contentOffset, contentSize }: NativeScrollEvent) => {
		const isScrolling = contentOffset.y > 0;
		setStartedScrolling(isScrolling);

		const hasReachedBottom = layoutMeasurement.height + contentOffset.y >= contentSize.height;
		setReachedBottom(hasReachedBottom);
	};

	return (
		<>
			<Modal
				ref={modalRef}
				onDismiss={() => setSelectedTags([])}
				enableContentPanningGesture={false}
				enablePanDownToClose={false}
				snapPoints={['50']}
				title="Select Tags"
			>
				{/* Back Button */}
				<Pressable
					onPress={() => modalRef.current?.close()}
					style={tw`absolute z-10 ml-6 rounded-full bg-app-button p-2`}
				>
					<CaretLeft color={tw.color('ink')} size={16} weight="bold" />
				</Pressable>
				<View
					onLayout={(e) => {
						if (e.nativeEvent.layout.height >= 80) {
							setReachedBottom(false);
						} else {
							setReachedBottom(true);
						}
					}}
					style={twStyle(`relative mt-4 h-[70%]`)}
				>
					<Fade
						fadeSides="top-bottom"
						orientation="vertical"
						color="bg-app-modal"
						width={20}
						topFadeStyle={twStyle(startedScrolling ? 'mt-0 h-6' : 'h-0')}
						bottomFadeStyle={twStyle(reachedBottom ? 'h-0' : 'h-6')}
						height="100%"
					>
						<FlatList
							data={tagsData}
							numColumns={3}
							onScroll={(e) => fadeScroll(e.nativeEvent)}
							extraData={selectedTags}
							key={tagsData ? 'tags' : '_'}
							keyExtractor={(item) => item.id.toString()}
							contentContainerStyle={tw`mx-auto p-4 pb-6`}
							ItemSeparatorComponent={() => <View style={tw`h-2`} />}
							renderItem={({ item }) => (
								<TagItem
									isSelected={() => isSelected(item.id)}
									select={() => selectTag(item.id)}
									tag={item}
								/>
							)}
						/>
					</Fade>
				</View>
				<View style={tw`flex-row gap-2 px-5`}>
					<Button
						onPress={() => newTagRef.current?.present()}
						style={tw`mb-10 h-10 flex-1 flex-row gap-1`}
						variant="dashed"
					>
						<Plus weight="bold" size={12} color={tw.color('text-ink-dull')} />
						<Text style={tw`text-sm font-medium text-ink-dull`}>Add New Tag</Text>
					</Button>
					<Button style={tw`mb-10 h-10 flex-1`} onPress={assignHandler} variant="accent">
						<Text style={tw`text-sm font-medium text-white`}>
							{appliedTags.length === 0 ? 'Confirm' : 'Update'}
						</Text>
					</Button>
				</View>
			</Modal>
			<CreateTagModal ref={newTagRef} />
		</>
	);
});

interface Props {
	tag: Tag;
	select: () => void;
	isSelected: () => boolean;
}

const TagItem = ({ tag, select, isSelected }: Props) => {
	return (
		<Pressable onPress={select}>
			<Card
				style={twStyle(`mr-2 w-auto flex-row items-center gap-2 border bg-app-card p-2`, {
					borderColor: isSelected() ? tw.color('accent') : tw.color('app-cardborder')
				})}
			>
				<View
					style={twStyle(`h-3.5 w-3.5 rounded-full`, {
						backgroundColor: tag.color!
					})}
				/>
				<Text style={tw`text-sm font-medium text-ink`}>{tag?.name}</Text>
			</Card>
		</Pressable>
	);
};

export default AddTagModal;
