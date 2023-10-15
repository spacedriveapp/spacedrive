import { Icon } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import { Icon as SDIcon } from '~/components';
import { getSearchStore, useKeybind } from '~/hooks';

// this could be handy elsewhere
export const RenderIcon = ({ icon }: { icon?: Icon | IconTypes }) => {
	if (typeof icon === 'string' && icon.startsWith('#')) {
		return (
			<div
				className="mr-0.5 h-[15px] w-[15px] shrink-0 rounded-full border"
				style={{
					backgroundColor: icon ? icon : 'transparent',
					borderColor: icon || '#efefef'
				}}
			/>
		);
	} else if (typeof icon === 'string') {
		return <SDIcon name={icon} size={20} className="text-ink-dull" />;
	} else {
		const IconComponent = icon;
		return (
			IconComponent && (
				<IconComponent
					size={16}
					weight="bold"
					className="text-ink-dull group-hover:text-white"
				/>
			)
		);
	}
};
