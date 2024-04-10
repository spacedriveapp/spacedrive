import { getIcon, iconNames } from '@sd/assets/util';
import { Image, ImageProps } from 'expo-image';
import { ClassInput } from 'twrnc';
import { isDarkTheme } from '@sd/client';
import { twStyle } from '~/lib/tailwind';

export type IconName = keyof typeof iconNames;

interface IconProps extends Omit<ImageProps, 'source' | 'style'> {
	name: IconName;
	size?: number;
	theme?: 'dark' | 'light';
	style?: ClassInput;
}

export const Icon = ({ name, size, theme, style, ...props }: IconProps) => {
	const isDark = isDarkTheme();

	return (
		<Image
			{...props}
			style={twStyle(style, {
				width: size,
				height: size
			})}
			source={getIcon(name, theme ? theme === 'dark' : isDark)}
		/>
	);
};
