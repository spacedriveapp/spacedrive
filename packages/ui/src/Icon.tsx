import { getIcon, iconNames } from '@sd/assets/util';
import { HTMLAttributes } from 'react';

interface Props extends HTMLAttributes<HTMLImageElement> {
	name: keyof typeof iconNames;
	size?: number;
	theme?: 'dark' | 'light';
}

export const Icon = ({ name, size, theme = 'dark', ...props }: Props) => {
	return <img src={getIcon(name, theme === 'dark')} width={size} height={size} {...props} />;
};
