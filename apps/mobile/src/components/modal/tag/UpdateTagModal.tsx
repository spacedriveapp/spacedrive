import { forwardRef, useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { Tag, useLibraryMutation } from '@sd/client';
import { FadeInAnimation } from '~/components/animation/layout';
import ColorPicker from '~/components/form/ColorPicker';
import { Input } from '~/components/form/Input';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw, twStyle } from '~/lib/tailwind';
import { useQueryClient } from '@tanstack/react-query';

type Props = {
	tag: Tag;
	onSubmit?: () => void;
};

const UpdateTagModal = forwardRef<ModalRef, Props>((props, ref) => {
	const queryClient = useQueryClient();
	const modalRef = useForwardedRef(ref);

	const [tagName, setTagName] = useState(props.tag.name!);
	const [tagColor, setTagColor] = useState(props.tag.color!);
	const [showPicker, setShowPicker] = useState(false);

	const { mutate: updateTag, isLoading } = useLibraryMutation('tags.update', {
		onSuccess: () => {
			// Reset form
			setShowPicker(false);

			queryClient.invalidateQueries(['tags.list']);

			props.onSubmit?.();
		},
		onSettled: () => {
			modalRef.current?.dismiss();
		}
	});

	useEffect(() => {
		modalRef.current?.snapToIndex(showPicker ? 1 : 0);
	}, [modalRef, showPicker]);

	return (
		<Modal
			ref={modalRef}
			snapPoints={['35%', '65%']}
			onDismiss={() => {
				// Resets form onDismiss
				setShowPicker(false);
			}}
			title="Update Tag"
			// Disable panning gestures
			enableHandlePanningGesture={false}
			enableContentPanningGesture={false}
			showCloseButton
		>
			<View style={tw`p-4`}>
				<Text style={tw`text-ink-dull mb-1 ml-1 text-xs font-medium`}>Name</Text>
				<Input value={tagName} onChangeText={(t) => setTagName(t)} />
				<Text style={tw`text-ink-dull mb-1 ml-1 mt-3 text-xs font-medium`}>Color</Text>
				<View style={tw`ml-2 flex flex-row items-center`}>
					<Pressable
						onPress={() => setShowPicker((v) => !v)}
						style={twStyle({ backgroundColor: tagColor }, 'h-5 w-5 rounded-full')}
					/>
					{/* TODO: Make this editable. Need to make sure color is a valid hexcode and update the color on picker etc. etc. */}
					<Input editable={false} value={tagColor as string} style={tw`ml-2 flex-1`} />
				</View>
				{showPicker && (
					<FadeInAnimation>
						<View style={tw`mt-4 h-64`}>
							<ColorPicker color={tagColor} onColorChangeComplete={(color) => setTagColor(color)} />
						</View>
					</FadeInAnimation>
				)}
				{/* TODO: Add loading to button */}
				<Button
					variant="accent"
					size="md"
					onPress={() => updateTag({ id: props.tag.id, color: tagColor, name: tagName })}
					style={tw`mt-6`}
					disabled={tagName.length === 0}
				>
					<Text style={tw`text-sm font-medium text-white`}>Save</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default UpdateTagModal;
