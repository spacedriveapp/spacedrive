import { getIcon, iconNames } from '@sd/assets/util';
import { ImgHTMLAttributes } from 'react';
import { useIsDark } from '~/hooks';

export type IconName = keyof typeof iconNames;

interface Props extends ImgHTMLAttributes<HTMLImageElement> {
	name: IconName;
	size?: number;
}

export const Icon = ({ name, size, ...props }: Props) => {
	const isDark = useIsDark();
	return <img src={getIcon(name, isDark)} width={size} height={size} {...props} />;
};
