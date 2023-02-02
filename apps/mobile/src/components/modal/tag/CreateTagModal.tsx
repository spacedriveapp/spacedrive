import { forwardRef, useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import ColorPicker from 'react-native-wheel-color-picker';
import { queryClient, useLibraryMutation } from '@sd/client';
import { FadeInAnimation } from '~/components/animation/layout';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import { Input } from '~/components/primitive/Input';
import useForwardedRef from '~/hooks/useForwardedRef';
import tw from '~/lib/tailwind';

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
			modalRef.current.dismiss();
		}
	});

	useEffect(() => {
		modalRef.current.snapToIndex(showPicker ? 1 : 0);
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
				<View style={tw`flex flex-row items-center mt-4`}>
					<Pressable
						onPress={() => setShowPicker((v) => !v)}
						style={tw.style({ backgroundColor: tagColor }, 'w-6 h-6 rounded-full')}
					/>
					<Input
						style={tw`flex-1 ml-2`}
						value={tagName}
						onChangeText={(text) => setTagName(text)}
						placeholder="Name"
					/>
				</View>
				{/* Color Picker */}
				{showPicker && (
					<FadeInAnimation>
						<View style={tw`h-64 mt-4`}>
							<ColorPicker
								autoResetSlider
								gapSize={0}
								thumbSize={40}
								sliderSize={24}
								shadeSliderThumb
								color={tagColor}
								onColorChangeComplete={(color) => setTagColor(color)}
								swatchesLast={false}
								palette={[
									tw.color('blue-500'),
									tw.color('red-500'),
									tw.color('green-500'),
									tw.color('yellow-500'),
									tw.color('purple-500'),
									tw.color('pink-500'),
									tw.color('gray-500'),
									tw.color('black'),
									tw.color('white')
								]}
							/>
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
					<Text style={tw`text-white font-medium text-sm`}>Create</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default CreateTagModal;
