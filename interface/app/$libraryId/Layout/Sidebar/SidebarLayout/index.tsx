import { SidebarSimple } from '@phosphor-icons/react';
import clsx from 'clsx';
import { motion, useAnimationControls, Variants } from 'framer-motion';
import { PropsWithChildren, useCallback, useEffect, useRef, useState } from 'react';
import { useKey } from 'rooks';
import { Button, Kbd, Resizable, ResizableHandle, Tooltip, useResizableContext } from '@sd/ui';
import { MacTrafficLights } from '~/components';
import {
	useKeyMatcher,
	useKeysMatcher,
	useLocale,
	useOperatingSystem,
	useShortcut,
	useShowControls
} from '~/hooks';

import { layoutStore, useLayoutStore } from '../../store';
import { getSidebarStore } from '../store';
import { SidebarContext } from './Context';
import Footer from './Footer';
import LibrariesDropdown from './LibrariesDropdown';

const TRANSITION_EASE = [0.25, 1, 0.5, 1];

export default ({ children }: PropsWithChildren) => {
	const os = useOperatingSystem();
	const showControls = useShowControls();

	const { sidebar } = useLayoutStore();

	// Prevent scroll with arrow up/down keys
	useKey(['ArrowUp', 'ArrowDown'], (e) => e.preventDefault());

	return (
		<Resizable
			min={176}
			max={300}
			initial={sidebar.size}
			collapsed={sidebar.collapsed}
			onCollapseChange={(val) => (layoutStore.sidebar.collapsed = val)}
			onResizeEnd={({ position }) => (layoutStore.sidebar.size = position)}
		>
			<div
				className={clsx(
					'bg-sidebar',
					(os === 'macOS' || showControls.transparentBg) && 'bg-opacity-[0.65]'
				)}
			>
				<SidebarSize />
				<SidebarContent>
					{showControls.isEnabled && <MacTrafficLights className="z-50 mb-1" />}

					<SidebarControls />

					<LibrariesDropdown />

					<div className="no-scrollbar mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
						{children}
						<div className="grow" />
					</div>

					<Footer />
				</SidebarContent>
			</div>
		</Resizable>
	);
};

const SidebarSize = () => {
	const resizable = useResizableContext();

	const controls = useAnimationControls();

	useEffect(() => {
		if (resizable.collapsed) return;
		controls.start({ width: resizable.position, transition: { duration: 0 } });
	}, [controls, resizable.position, resizable.collapsed]);

	useEffect(() => {
		controls.start({ width: resizable.size, transition: { ease: TRANSITION_EASE } });
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [controls, resizable.collapsed]);

	return <motion.div initial={{ width: resizable.size }} animate={controls} />;
};

const SidebarContent = ({ children }: PropsWithChildren) => {
	const resizable = useResizableContext();

	const ref = useRef<HTMLDivElement>(null);

	const [show, setShow] = useState(!resizable.collapsed);

	// Used to prevent any changes to the open state
	// e.g. when popovers are open from the sidebar
	const [locked, setLocked] = useState(false);

	const [hovered, setHovered] = useState(false);
	const [focused, setFocused] = useState(false);

	const controls = useAnimationControls();

	const variants: Variants = {
		hide: { left: -resizable.position + 12, transition: { ease: TRANSITION_EASE } },
		show: { left: 0, transition: { ease: TRANSITION_EASE } }
	};

	const toggleSidebar = useCallback(
		(show: boolean) => {
			controls.start(show ? 'show' : 'hide');
			setShow(show);
		},
		[controls]
	);

	useEffect(() => {
		const node = ref.current;
		if (!node || !resizable.collapsed) return;

		const handleMouseMove = (e: MouseEvent) => {
			const { clientX, clientY } = e;
			const rect = node.getBoundingClientRect();

			const isHoveringX = clientX >= rect.left && clientX <= rect.right;
			const isHoveringY = clientY >= rect.top && clientY <= rect.bottom;

			setHovered(isHoveringX && isHoveringY);
		};

		window.addEventListener('mousemove', handleMouseMove);
		return () => window.removeEventListener('mousemove', handleMouseMove);
	}, [resizable.collapsed]);

	useEffect(() => {
		const node = ref.current;
		if (!node || !resizable.collapsed) return;

		const handleFocus = (focused: boolean) => setFocused(focused);

		node.addEventListener('focusin', () => handleFocus(true));
		node.addEventListener('focusout', () => handleFocus(false));

		return () => {
			node.removeEventListener('focusin', () => handleFocus(true));
			node.removeEventListener('focusout', () => handleFocus(false));
		};
	}, [resizable.collapsed]);

	useEffect(() => {
		if (!resizable.collapsed || resizable.isDragging || hovered || locked) return;
		toggleSidebar(focused);
	}, [focused, hovered, resizable.collapsed, resizable.isDragging, toggleSidebar, locked]);

	useEffect(() => {
		if (!resizable.collapsed) return;
		setTimeout(() => toggleSidebar(locked));
	}, [toggleSidebar, locked, resizable.collapsed]);

	useEffect(() => {
		if (!resizable.collapsed || resizable.isDragging || locked) return;
		// Timeout toggle as LibrariesDropdown triggers a focus on close which causes a flicker
		setTimeout(() => toggleSidebar(hovered));
	}, [hovered, resizable.collapsed, resizable.isDragging, toggleSidebar, locked]);

	useEffect(() => {
		setTimeout(() => toggleSidebar(!resizable.collapsed));

		// Temporary solution until we have a better way to handle pinned popovers
		if (resizable.collapsed) getSidebarStore().pinJobManager = false;

		setLocked(false);
		setHovered(false);
		setFocused(false);
	}, [resizable.collapsed, toggleSidebar]);

	useShortcut('toggleSidebar', () => (layoutStore.sidebar.collapsed = !resizable.collapsed), {
		disabled: locked || resizable.isDragging
	});

	return (
		<SidebarContext.Provider
			value={{ show, locked, onLockedChange: setLocked, collapsed: resizable.collapsed }}
		>
			<motion.div
				ref={ref}
				initial={show ? 'show' : 'hide'}
				animate={controls}
				variants={variants}
				className={clsx('fixed inset-y-0 z-[100]', resizable.collapsed && 'p-1 pr-3')}
				style={{
					// We add 16px from the padding on the x-axis
					width: resizable.position + (show && resizable.collapsed ? 16 : 0)
				}}
			>
				<nav
					className={clsx(
						'relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pb-2',
						// Uncomment if SidebarControls are removed
						// 'transition-[padding-top] ease-linear motion-reduce:transition-none',
						// os === 'macOS' && !windowState.isFullScreen && 'pt-2.5',
						resizable.collapsed
							? 'rounded-md border border-app-line bg-sidebar shadow'
							: null
					)}
				>
					{children}
					<ResizeHandle />
				</nav>
			</motion.div>
		</SidebarContext.Provider>
	);
};

function SidebarControls() {
	const { t } = useLocale();
	const os = useOperatingSystem();
	const { sidebar } = useLayoutStore();
	const ctrlmeta = useKeysMatcher(['Meta']).Meta.icon;

	if (os !== 'macOS') return null;

	return (
		<div className="flex justify-end">
			<Tooltip
				label={!sidebar.collapsed ? t('hide_sidebar') : t('lock_sidebar')}
				keybinds={[ctrlmeta, 'S']}
			>
				<Button
					size="icon"
					onClick={() => (layoutStore.sidebar.collapsed = !layoutStore.sidebar.collapsed)}
				>
					<SidebarSimple className="size-[18px]" />
				</Button>
			</Tooltip>
		</div>
	);
}

function ResizeHandle() {
	const { t } = useLocale();
	const { sidebar } = useLayoutStore();
	const resizable = useResizableContext();

	const [cursor, setCursor] = useState<{ x: number; y: number }>();
	const [collapse, setCollapse] = useState(false);

	useEffect(() => setCollapse(false), [resizable.position]);

	return (
		<Tooltip
			align="start"
			position="right"
			disableHoverableContent
			alignOffset={(cursor?.y ?? 0) - (sidebar.collapsed ? 39 : 35)}
			className={clsx('!absolute inset-y-2 -right-1')}
			label={
				resizable.isDragging ? null : (
					<div className="flex flex-col items-start">
						<div>{t('drag_to_resize')}</div>
						<div className="flex items-center gap-1">
							<span>{t(sidebar.collapsed ? 'click_to_lock' : 'click_to_hide')}</span>
							<Kbd>[</Kbd>
						</div>
					</div>
				)
			}
		>
			<ResizableHandle
				className="h-full after:rounded-full"
				onMouseOver={(e) => setCursor({ x: e.clientX, y: e.clientY })}
				onMouseDown={() => setCollapse(true)}
				onMouseUp={() => collapse && (layoutStore.sidebar.collapsed = !sidebar.collapsed)}
			/>
		</Tooltip>
	);
}
