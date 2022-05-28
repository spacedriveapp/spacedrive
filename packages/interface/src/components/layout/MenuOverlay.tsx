import { ContextMenu, ContextMenuProps } from '@sd/ui';
import clsx from 'clsx';
import React, { MouseEventHandler, useLayoutEffect } from 'react';

type MenuElement = React.ReactElement<{ style?: React.CSSProperties; className?: string }>;
type Position = {
	x: number;
	y: number;
};

export interface MenuContextData {
	currentMenu?: {
		clickPosition: Position;
		clickedElement: HTMLElement;
		menuElement: MenuElement;
	};
}

export interface MenuContextActions {
	showMenu: (menu: MenuElement, clickPosition: Position, clickedElement: HTMLElement) => void;
	dismiss: () => void;
}

export const MenuContext = React.createContext<MenuContextData & MenuContextActions>({
	showMenu() {},
	dismiss() {}
});

export const useMenu = () => React.useContext(MenuContext);

export const WithContextMenu: React.FC<{
	menu: ContextMenuProps['items'];
	children: React.ReactElement<{ onContextMenu: MouseEventHandler }>;
}> = (props) => {
	const { menu: sections = [], children } = props;

	const menu = useMenu();

	return (
		<>
			{React.isValidElement(children) &&
				React.cloneElement(children, {
					onContextMenu(e: React.MouseEvent) {
						e.preventDefault();
						e.stopPropagation();

						menu.showMenu(
							<ContextMenu items={sections} />,
							{ x: e.clientX, y: e.clientY },
							e.target as HTMLElement
						);
					}
				})}
		</>
	);
};

export const MenuOverlay: React.FC<{ children: React.ReactNode }> = (props) => {
	const { children } = props;

	const [menuState, setMenuState] = React.useState<MenuContextData>({});

	const overlay = React.useRef<HTMLDivElement>(null);

	const showMenu: MenuContextActions['showMenu'] = React.useCallback(
		(menu, clickPosition, clickedElement) => {
			setMenuState({
				currentMenu: {
					menuElement: menu,
					clickPosition,
					clickedElement
				}
			});
		},
		[setMenuState]
	);

	const dismiss: MenuContextActions['dismiss'] = React.useCallback(() => {
		setMenuState({});
	}, [setMenuState]);

	useLayoutEffect(() => {
		if (menuState.currentMenu) overlay.current?.focus();
		else overlay.current?.blur();
	}, [menuState]);

	return (
		<MenuContext.Provider
			value={{
				showMenu,
				dismiss,
				currentMenu: menuState.currentMenu
			}}
		>
			{children}
			<div
				className={clsx('absolute top-0 left-0 w-screen h-screen pointer-events-none', {
					'pointer-events-auto': menuState.currentMenu
				})}
				ref={overlay}
				onKeyDownCapture={(e) => {
					if (e.key === 'Escape') {
						e.stopPropagation();

						setMenuState({});
					}
				}}
				onClick={() => {
					setMenuState({});
				}}
				onContextMenu={(e) => {
					e.preventDefault();
				}}
			>
				{menuState.currentMenu && React.isValidElement(menuState.currentMenu?.menuElement) && (
					<div className="relative">
						{React.cloneElement(menuState.currentMenu!.menuElement, {
							className: 'absolute',
							style: {
								left: menuState.currentMenu?.clickPosition.x + 3,
								top: menuState.currentMenu?.clickPosition.y + 3
							}
						})}
					</div>
				)}
			</div>
		</MenuContext.Provider>
	);
};
