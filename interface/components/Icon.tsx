import { getIcon, iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import { ImgHTMLAttributes } from 'react';
import { useIsDark } from '~/hooks';

export type IconName = keyof typeof iconNames;

interface Props extends ImgHTMLAttributes<HTMLImageElement> {
	name: IconName;
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
			className={clsx('pointer-events-none', props.className)}
		/>
	);
};
