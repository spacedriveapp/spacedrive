import { getIcon, iconNames } from '@sd/assets/util';
import { HTMLAttributes } from 'react';
import { useIsDark } from '~/hooks';

interface Props extends HTMLAttributes<HTMLImageElement> {
	name: keyof typeof iconNames;
	size?: number;
	theme?: 'dark' | 'light';
}

export const Icon = ({ name, size, theme, ...props }: Props) => {
	const isDark = useIsDark();
	return (
		<img
			src={getIcon(name, theme ? theme === 'dark' : isDark)}
			width={size}
			height={size}
			{...props}
		/>
	);
};
