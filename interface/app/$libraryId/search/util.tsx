import { CircleDashed, Folder, Icon, Tag } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import i18n from '~/app/I18n';
import { Icon as SDIcon } from '~/components';

export const RenderIcon = ({
	className,
	icon
}: {
	icon?: Icon | IconTypes | string;
	className?: string;
}) => {
	if (typeof icon === 'string' && icon.startsWith('#')) {
		return (
			<div
				className={clsx('mr-0.5 size-[15px] shrink-0 rounded-full border', className)}
				style={{
					backgroundColor: icon ? icon : 'transparent',
					borderColor: icon || '#efefef'
				}}
			/>
		);
	} else if (typeof icon === 'string') {
		return (
			<SDIcon
				name={icon as any}
				size={18}
				className={clsx('shrink-0 text-ink-dull', className)}
			/>
		);
	} else {
		const IconComponent = icon;
		return (
			IconComponent && (
				<IconComponent
					size={15}
					weight="bold"
					className={clsx('shrink-0 text-ink-dull', className)}
				/>
			)
		);
	}
};

export const getIconComponent = (iconName: string): Icon => {
	const icons: Record<string, React.ComponentType> = {
		Folder,
		CircleDashed,
		Tag
	};

	return icons[iconName] as Icon;
};
