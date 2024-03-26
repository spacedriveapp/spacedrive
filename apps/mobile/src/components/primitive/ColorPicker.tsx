import React from 'react';
import WheelColorPicker from 'react-native-wheel-color-picker';
import { tw } from '~/lib/tailwind';

type ColorPickerProps = {
	color: string;
	onColorChangeComplete: (color: string) => void;
};

const defaultPalette = [
	tw.color('blue-500'),
	tw.color('red-500'),
	tw.color('green-500'),
	tw.color('yellow-500'),
	tw.color('purple-500'),
	tw.color('pink-500'),
	tw.color('gray-500'),
	tw.color('black'),
	tw.color('white')
];

const ColorPicker = ({ color, onColorChangeComplete }: ColorPickerProps) => {
	return (
		<WheelColorPicker
			autoResetSlider
			gapSize={0}
			thumbSize={40}
			sliderSize={24}
			shadeSliderThumb
			color={color}
			onColorChangeComplete={onColorChangeComplete}
			swatchesLast={false}
			palette={defaultPalette as string[]}
		/>
	);
};

export default ColorPicker;
