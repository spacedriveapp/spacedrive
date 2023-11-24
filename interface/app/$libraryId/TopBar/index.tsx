import { Plus, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useLayoutEffect, useRef } from 'react';
import { useKey } from 'rooks';
import useResizeObserver from 'use-resize-observer';
import { Tooltip } from '@sd/ui';
import { useKeyMatcher, useOperatingSystem, useShowControls } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';
import { useTabsContext } from '~/TabsContext';

import { useExplorerStore } from '../Explorer/store';
import { useTopBarContext } from './Layout';
import { NavigationButtons } from './NavigationButtons';

const TopBar = () => {
	const transparentBg = useShowControls().transparentBg;
	const { isDragging } = useExplorerStore();
	const ref = useRef<HTMLDivElement>(null);

	const tabs = useTabsContext();
	const ctx = useTopBarContext();

	useResizeObserver({
		ref,
		box: 'border-box',
		onResize(bounds) {
			if (bounds.height === undefined) return;
			ctx.setTopBarHeight(bounds.height);
		}
	});

	// when the component mounts + crucial state changes, we need to update the height _before_ the browser paints
	// in order to avoid jank. resize observer doesn't fire early enought to account for this.
	useLayoutEffect(() => {
		const height = ref.current!.getBoundingClientRect().height;
		ctx.setTopBarHeight.call(undefined, height);
	}, [ctx.setTopBarHeight]);

	return (
		<div
			ref={ref}
			data-tauri-drag-region
			className={clsx(
				'top-bar-blur absolute inset-x-0 z-50 border-b border-sidebar-divider',
				transparentBg ? 'bg-app/0' : 'bg-app/90'
			)}
		>
			<div
				data-tauri-drag-region
				className={clsx(
					'flex h-12 items-center gap-3.5 overflow-hidden px-3.5',
					'duration-250 transition-[background-color,border-color] ease-out',
					isDragging && 'pointer-events-none'
				)}
			>
				<div
					data-tauri-drag-region
					className="flex flex-1 items-center gap-3.5 overflow-hidden"
				>
					<NavigationButtons />
					<div ref={ctx.setLeft} className="overflow-hidden" />
				</div>

				<div ref={ctx.setCenter} />

				<div ref={ctx.setRight} className="flex-1" />
			</div>

			{tabs && <Tabs />}

			<div ref={ctx.setChildren} />
		</div>
	);
};

export default TopBar;

function Tabs() {
	const ctx = useTabsContext()!;
	const keybind = useKeyMatcher('Meta');

	function addTab() {
		ctx.createTab();
	}

	function removeTab(index: number) {
		if (ctx.tabs.length === 1) return;

		ctx.removeTab(index);
	}

	useTabKeybinds({ addTab, removeTab });

	if (ctx.tabs.length < 2) return null;

	return (
		<div
			data-tauri-drag-region
			className="no-scrollbar flex h-9 w-full flex-row items-center divide-x divide-sidebar-divider overflow-x-auto text-xs text-ink-dull"
		>
			{ctx.tabs.map(({ title }, index) => (
				<button
					onClick={(e) => {
						if (e.button === 0) ctx.setTabIndex(index);
						else if (e.button === 1) removeTab(index);
					}}
					className={clsx(
						'duration-[50ms] group relative flex h-full min-w-[10rem] shrink-0 flex-row items-center justify-center px-8 text-center',
						ctx.tabIndex === index
							? 'text-ink'
							: 'top-bar-blur bg-sidebar transition-colors hover:bg-app/50'
					)}
					key={index}
				>
					{title}
					{ctx.tabs.length > 1 && (
						<div
							onClick={(e) => {
								e.stopPropagation();
								removeTab(index);
							}}
							className="absolute right-2 rounded p-1 opacity-0 transition-opacity hover:bg-app-selected group-hover:opacity-100"
						>
							<X />
						</div>
					)}
				</button>
			))}
			<div
				className="flex h-full flex-1 items-center justify-start bg-sidebar px-2"
				data-tauri-drag-region
			>
				<Tooltip keybinds={[keybind.icon, 'T']} label="New Tab">
					<button
						onClick={addTab}
						className="duration-[50ms] flex flex-row items-center justify-center rounded p-1.5 transition-colors hover:bg-app/80"
					>
						<Plus weight="bold" size={14} />
					</button>
				</Tooltip>
			</div>
		</div>
	);
}

function useTabKeybinds(props: { addTab(): void; removeTab(index: number): void }) {
	const ctx = useTabsContext()!;
	const os = useOperatingSystem();
	const { visible } = useRoutingContext();

	// these keybinds aren't part of the regular shortcuts system as they're desktop-only
	useKey(['t'], (e) => {
		if (!visible) return;
		if ((os === 'macOS' && !e.metaKey) || (os !== 'macOS' && !e.ctrlKey)) return;

		e.stopPropagation();

		props.addTab();
	});

	useKey(['w'], (e) => {
		if (!visible) return;
		if ((os === 'macOS' && !e.metaKey) || (os !== 'macOS' && !e.ctrlKey)) return;

		e.stopPropagation();

		props.removeTab(ctx.tabIndex);
	});

	useKey(['ArrowLeft', 'ArrowRight'], (e) => {
		if (!visible) return;
		// TODO: figure out non-macos keybind
		if ((os === 'macOS' && !(e.metaKey && e.altKey)) || os !== 'macOS') return;

		e.stopPropagation();

		const delta = e.key === 'ArrowLeft' ? -1 : 1;

		ctx.setTabIndex(Math.min(Math.max(0, ctx.tabIndex + delta), ctx.tabs.length - 1));
	});
}
