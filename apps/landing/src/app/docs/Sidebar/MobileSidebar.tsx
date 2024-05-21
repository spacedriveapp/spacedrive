'use client';

import { List, X } from '@phosphor-icons/react';
import {
	createContext,
	Dispatch,
	PropsWithChildren,
	SetStateAction,
	useContext,
	useState
} from 'react';
import { slide as Menu } from 'react-burger-menu';
import { Button } from '@sd/ui';

const MenuContext = createContext<{
	open: boolean;
	setOpen: Dispatch<SetStateAction<boolean>>;
} | null>(null);

export function MobileSidebarProvider({ children }: PropsWithChildren) {
	const [open, setOpen] = useState(false);

	return <MenuContext.Provider value={{ open, setOpen }}>{children}</MenuContext.Provider>;
}

export function useMenuContext() {
	const ctx = useContext(MenuContext);

	if (!ctx) throw new Error('useMenuContext must be used within a MenuProvider');

	return ctx;
}

export function OpenMobileSidebarButton() {
	const menu = useMenuContext();

	return (
		<Button className="ml-1 !border-none !px-2" onClick={() => menu.setOpen((o) => !o)}>
			<List weight="bold" className="size-6" />
		</Button>
	);
}

export function MobileSidebarWrapper({ children }: PropsWithChildren) {
	const menu = useMenuContext();

	return (
		<Menu
			onClose={() => menu.setOpen(false)}
			customBurgerIcon={false}
			isOpen={menu.open}
			pageWrapId="page-container"
			className="shadow-2xl shadow-black"
		>
			<div className="custom-scroll doc-sidebar-scroll visible h-screen overflow-x-hidden bg-gray-650 px-7 pb-20 pt-7 sm:invisible">
				<Button
					onClick={() => menu.setOpen((o) => !o)}
					className="-ml-0.5 mb-3 !border-none !px-1"
				>
					<X weight="bold" className="size-6" />
				</Button>
				{children}
			</div>
		</Menu>
	);
}
