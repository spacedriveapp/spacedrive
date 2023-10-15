import { Icon } from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import { Icon as SDIcon } from '~/components';
import { getSearchStore, useKeybind } from '~/hooks';

// this could be handy elsewhere
export const RenderIcon = ({ icon }: { icon?: Icon | IconTypes }) => {
	if (typeof icon === 'string') {
		return <SDIcon name={icon} size={16} className="mr-2 text-ink-dull" />;
	} else if (typeof icon === 'function') {
		const IconComponent = icon;
		return (
			<IconComponent
				size={16}
				weight="bold"
				className="mr-2 text-ink-dull group-hover:text-white"
			/>
		);
	}
	return null;
};
