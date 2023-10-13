import { getIcon, iconNames } from '@sd/assets/util';
import { HTMLAttributes } from 'react';
import { useIsDark } from '~/hooks';

interface Props extends HTMLAttributes<HTMLImageElement> {
	name: keyof typeof iconNames;
	size?: number;
}

export const Icon = ({ name, size, ...props }: Props) => {
	const isDark = useIsDark();
	return <img src={getIcon(name, isDark)} width={size} height={size} {...props} />;
};
