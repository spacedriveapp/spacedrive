import { getIcon, iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import Image from 'next/image';
import { ImgHTMLAttributes } from 'react';

export type IconName = keyof typeof iconNames;

interface Props extends ImgHTMLAttributes<HTMLImageElement> {
	name: IconName;
	size?: number;
}

export const Icon = ({ name, size, ...props }: Props) => {
	return (
		<Image
			src={getIcon(name, true)}
			width={size}
			height={size}
			alt={name}
			className={clsx('pointer-events-none', props.className)}
		/>
	);
};
