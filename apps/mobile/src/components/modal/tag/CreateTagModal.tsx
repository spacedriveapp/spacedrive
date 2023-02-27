import { forwardRef, useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import ColorPicker from 'react-native-wheel-color-picker';
import { queryClient, useLibraryMutation } from '@sd/client';
import { FadeInAnimation } from '~/components/animation/layout';
import { Input } from '~/components/form/Input';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw, twStyle } from '~/lib/tailwind';

const CreateTagModal = forwardRef<ModalRef, unknown>((_, ref) => {
	const modalRef = useForwardedRef(ref);

	const [tagName, setTagName] = useState('');
	const [tagColor, setTagColor] = useState('#A717D9');
	const [showPicker, setShowPicker] = useState(false);

	// TODO: Use react-hook-form?

	const { mutate: createTag } = useLibraryMutation('tags.create', {
		onSuccess: () => {
			// Reset form
			setTagName('');
			setTagColor('#A717D9');
			setShowPicker(false);

			queryClient.invalidateQueries(['tags.list']);
		},
		onSettled: () => {
			// Close modal
			modalRef.current?.dismiss();
		}
	});

	useEffect(() => {
		modalRef.current?.snapToIndex(showPicker ? 1 : 0);
	}, [modalRef, showPicker]);

	return (
		<Modal
			ref={modalRef}
			snapPoints={['30%', '60%']}
			title="Create Tag"
			onDismiss={() => {
				// Resets form onDismiss
				setTagName('');
				setTagColor('#A717D9');
				setShowPicker(false);
			}}
			// Disable panning gestures
			enableHandlePanningGesture={false}
			enableContentPanningGesture={false}
			showCloseButton
		>
			<View style={tw`p-4`}>
				<View style={tw`mt-4 flex flex-row items-center`}>
					<Pressable
						onPress={() => setShowPicker((v) => !v)}
						style={twStyle({ backgroundColor: tagColor }, 'h-6 w-6 rounded-full')}
					/>
					<Input
						style={tw`ml-2 flex-1`}
						value={tagName}
						onChangeText={(text) => setTagName(text)}
						placeholder="Name"
					/>
				</View>
				{/* Color Picker */}
				{showPicker && (
					<FadeInAnimation>
						<View style={tw`mt-4 h-64`}>
							<ColorPicker color={tagColor} onColorChangeComplete={(color) => setTagColor(color)} />
						</View>
					</FadeInAnimation>
				)}
				<Button
					variant="accent"
					size="md"
					onPress={() => createTag({ color: tagColor, name: tagName })}
					style={tw`mt-6`}
					disabled={tagName.length === 0}
				>
					<Text style={tw`text-sm font-medium text-white`}>Create</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default CreateTagModal;
