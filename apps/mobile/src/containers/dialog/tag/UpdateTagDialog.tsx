import React, { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import ColorPicker from 'react-native-wheel-color-picker';
import { Tag, queryClient, useLibraryMutation } from '@sd/client';
import Dialog from '~/components/layout/Dialog';
import { Input } from '~/components/primitive/Input';
import tw from '~/lib/tailwind';

type Props = {
	tag: Tag;
	onSubmit?: () => void;
	children: React.ReactNode;
};

const UpdateTagDialog = ({ children, onSubmit, tag }: Props) => {
	const [tagName, setTagName] = useState(tag.name);
	const [tagColor, setTagColor] = useState(tag.color);
	const [isOpen, setIsOpen] = useState(false);

	const { mutate: updateTag, isLoading } = useLibraryMutation('tags.update', {
		onSuccess: () => {
			// Reset form
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
			title="Update Tag"
			ctaLabel="Save"
			ctaAction={() => updateTag({ id: tag.id, color: tagColor, name: tagName })}
			loading={isLoading}
			ctaDisabled={tagName.length === 0}
			trigger={children}
			onClose={() => {
				setShowPicker(false); // Reset form
			}}
		>
			<Text style={tw`mb-1 ml-1 mt-3 text-xs font-medium text-ink-dull`}>Name</Text>
			<Input value={tagName} onChangeText={(t) => setTagName(t)} />
			<Text style={tw`mb-1 ml-1 mt-3 text-xs font-medium text-ink-dull`}>Color</Text>
			<View style={tw`ml-2 flex flex-row items-center`}>
				<Pressable
					onPress={() => setShowPicker((v) => !v)}
					style={tw.style({ backgroundColor: tagColor }, 'w-5 h-5 rounded-full')}
				/>
				{/* TODO: Make this editable. Need to make sure color is a valid hexcode and update the color on picker etc. etc. */}
				<Input editable={false} value={tagColor} style={tw`ml-2 flex-1`} />
			</View>

			{showPicker && (
				<View style={tw`mt-4 h-64`}>
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

export default UpdateTagDialog;
