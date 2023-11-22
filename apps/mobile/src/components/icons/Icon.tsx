import { getIcon, iconNames } from '@sd/assets/util';
import { Image, ImageProps } from 'react-native';
import { isDarkTheme } from '@sd/client';

export type IconName = keyof typeof iconNames;

interface IconProps extends Omit<ImageProps, 'source'> {
	name: IconName;
	size?: number;
	theme?: 'dark' | 'light';
}

export const Icon = ({ name, size, theme, ...props }: IconProps) => {
	const isDark = isDarkTheme();

	return (
		<Image
			{...props}
			width={size}
			height={size}
			source={getIcon(name, theme ? theme === 'dark' : isDark)}
		/>
	);
};
