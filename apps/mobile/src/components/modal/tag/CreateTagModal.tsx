import { forwardRef, useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import ColorPicker from 'react-native-wheel-color-picker';
import {
	ToastDefautlColor,
	useLibraryMutation,
	usePlausibleEvent,
	useRspcLibraryContext
} from '@sd/client';
import { FadeInAnimation } from '~/components/animation/layout';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import { ModalInput } from '~/components/primitive/Input';
import { toast } from '~/components/primitive/Toast';
import useForwardedRef from '~/hooks/useForwardedRef';
import { useKeyboard } from '~/hooks/useKeyboard';
import { tw, twStyle } from '~/lib/tailwind';

const CreateTagModal = forwardRef<ModalRef, unknown>((_, ref) => {
	const rspc = useRspcLibraryContext();
	const modalRef = useForwardedRef(ref);

	const [tagName, setTagName] = useState('');
	const [tagColor, setTagColor] = useState(ToastDefautlColor);
	const [showPicker, setShowPicker] = useState(false);

	// TODO: Use react-hook-form?

	const submitPlausibleEvent = usePlausibleEvent();

	const { mutate: createTag } = useLibraryMutation('tags.create', {
		onSuccess: () => {
			// Reset form
			setTagName('');
			setTagColor(ToastDefautlColor);
			setShowPicker(false);

			rspc.queryClient.invalidateQueries(['tags.list']);

			toast.success('Tag created successfully');
			submitPlausibleEvent({ event: { type: 'tagCreate' } });
		},
		onError: (error) => {
			toast.error(error.message);
		},
		onSettled: () => {
			// Close modal
			modalRef.current?.dismiss();
		}
	});

	const { keyboardShown } = useKeyboard();

	useEffect(() => {
		if (!keyboardShown && showPicker) {
			modalRef.current?.snapToPosition('58');
		} else if (keyboardShown && showPicker) {
			modalRef.current?.snapToPosition('94');
		}
	}, [keyboardShown, modalRef, showPicker]);

	return (
		<Modal
			ref={modalRef}
			snapPoints={['22']}
			title="Create Tag"
			onDismiss={() => {
				// Resets form onDismiss
				setTagName('');
				setTagColor(ToastDefautlColor);
				setShowPicker(false);
			}}
		>
			<View style={tw`p-4`}>
				<View style={tw`mt-2 flex flex-row items-center`}>
					<Pressable
						onPress={() => setShowPicker(true)}
						style={twStyle({ backgroundColor: tagColor }, 'h-6 w-6 rounded-full')}
					/>
					<ModalInput
						autoFocus
						style={tw`ml-2 flex-1`}
						value={tagName}
						onChangeText={(text) => setTagName(text)}
						placeholder="Name"
					/>
				</View>
				{/* Color Picker */}
				{showPicker && (
					<FadeInAnimation>
						<View style={tw`my-4 h-64`}>
							<ColorPicker
								color={tagColor}
								onColorChangeComplete={(color) => setTagColor(color)}
							/>
						</View>
					</FadeInAnimation>
				)}
				<Button
					variant="accent"
					onPress={() => createTag({ color: tagColor, name: tagName })}
					style={tw`mt-2`}
					disabled={tagName.length === 0}
				>
					<Text style={tw`text-sm font-medium text-white`}>Create</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default CreateTagModal;
