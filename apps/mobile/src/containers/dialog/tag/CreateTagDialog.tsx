import { useQueryClient } from '@tanstack/react-query';
import React, { useState } from 'react';
import { Pressable, View } from 'react-native';
import ColorPicker from 'react-native-wheel-color-picker';
import { useLibraryMutation } from '@sd/client';
import Dialog from '~/components/layout/Dialog';
import { Input } from '~/components/primitive/Input';
import tw from '~/lib/tailwind';

type Props = {
	onSubmit?: () => void;
	disableBackdropClose?: boolean;
	children: React.ReactNode;
};

const CreateTagDialog = ({ children, onSubmit, disableBackdropClose }: Props) => {
	const queryClient = useQueryClient();
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

			onSubmit?.();
		},
		onSettled: () => {
			// Close dialog
			setIsOpen(false);
		}
	});

	const [showPicker, setShowPicker] = useState(false);

	return (
		<Dialog
			isVisible={isOpen}
			setIsVisible={setIsOpen}
			title="Create New Tag"
			description="Choose a name and color."
			ctaLabel="Create"
			ctaAction={() => createTag({ color: tagColor, name: tagName })}
			loading={isLoading}
			ctaDisabled={tagName.length === 0}
			trigger={children}
			disableBackdropClose={disableBackdropClose}
			onClose={() => {
				setTagName('');
				setTagColor('#A717D9');
				setShowPicker(false);
			}} // Resets form onClose
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
		</Dialog>
	);
};

export default CreateTagDialog;
