import { useMemo } from 'react';
import { ContextMenu, DropdownMenu, useContextMenuContext, useDropdownMenuContext } from '@sd/ui';

export const useMenu = (): typeof DropdownMenu | typeof ContextMenu | undefined => {
	const isDropdownMenu = useDropdownMenuContext();
	const isContextMenu = useContextMenuContext();

	const menu = useMemo(
		() => (isDropdownMenu ? DropdownMenu : isContextMenu ? ContextMenu : undefined),
		[isDropdownMenu, isContextMenu]
	);

	return menu;
};

const Separator = (
	props: Parameters<typeof ContextMenu.Separator | typeof DropdownMenu.Separator>[0]
) => {
	const Menu = useMenu();

	if (!Menu) return null;

	return <Menu.Separator {...props} />;
};

const SubMenu = (
	props: Parameters<typeof ContextMenu.SubMenu | typeof DropdownMenu.SubMenu>[0]
) => {
	const Menu = useMenu();

	if (!Menu) return null;

	return <Menu.SubMenu {...props} />;
};

const Item = (props: Parameters<typeof ContextMenu.Item | typeof DropdownMenu.Item>[0]) => {
	const ContextMenu = useMenu();

	if (!ContextMenu) return null;

	return <ContextMenu.Item {...props} />;
};

export const Menu = {
	Item,
	Separator,
	SubMenu
};
