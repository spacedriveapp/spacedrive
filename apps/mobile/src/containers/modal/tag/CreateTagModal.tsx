import { BottomSheetModal } from '@gorhom/bottom-sheet';
import { forwardRef, useState } from 'react';
import { Pressable, View } from 'react-native';
import ColorPicker from 'react-native-wheel-color-picker';
import { queryClient, useLibraryMutation } from '@sd/client';
import { Modal } from '~/components/layout/Modal';
import { Input } from '~/components/primitive/Input';
import useForwardedRef from '~/hooks/useForwardedRef';
import tw from '~/lib/tailwind';

// TODO: Needs styling
const CreateTagModal = forwardRef<BottomSheetModal, unknown>((_, ref) => {
	const modalRef = useForwardedRef(ref);

	const [tagName, setTagName] = useState('');
	const [tagColor, setTagColor] = useState('#A717D9');
	const [isOpen, setIsOpen] = useState(false);

	const { mutate: createTag, isLoading } = useLibraryMutation('tags.create', {
		onSuccess: () => {
			// Reset form
			setTagName('');
			setTagColor('#A717D9');
			setShowPicker(false);

			queryClient.invalidateQueries(['tags.list']);
		},
		onSettled: () => {
			// Close dialog
			setIsOpen(false);
		}
	});

	const [showPicker, setShowPicker] = useState(false);

	return (
		<Modal
			ref={modalRef}
			snapPoints={['40%', '60%']}
			onDismiss={() => {
				// Resets form onDismiss
				setTagName('');
				setTagColor('#A717D9');
				setShowPicker(false);
			}}
		>
			<View style={tw`flex flex-row items-center`}>
				<Pressable
					onPress={() => setShowPicker((v) => !v)}
					style={tw.style({ backgroundColor: tagColor }, 'w-5 h-5 rounded-full')}
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
			)}
			{/* Button */}
		</Modal>
	);
});

export default CreateTagModal;
