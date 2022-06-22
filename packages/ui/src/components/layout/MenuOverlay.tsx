import { ContextMenu, ContextMenuProps, Root, Trigger } from '@sd/ui';
import React, { ComponentProps } from 'react';

export const WithContextMenu: React.FC<{
	menu: ContextMenuProps['items'];
	children: ComponentProps<typeof Trigger>['children'];
}> = (props) => {
	const { menu: sections = [], children } = props;

	return (
		<Root>
			<Trigger>{children}</Trigger>

			<ContextMenu items={sections} />
		</Root>
	);
};
