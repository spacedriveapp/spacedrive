import { Plus, X } from '@phosphor-icons/react';
import { useSelector } from '@sd/client';
import { Tooltip } from '@sd/ui';
import clsx from 'clsx';
import { useLayoutEffect, useRef } from 'react';
import useResizeObserver from 'use-resize-observer';
import { useRoutingContext } from '~/RoutingContext';
import { useTabsContext } from '~/TabsContext';
import {
	useKeyMatcher,
	useLocale,
	useOperatingSystem,
	useShortcut,
	useShowControls
} from '~/hooks';

import { explorerStore } from '../Explorer/store';
import { useTopBarContext } from './Layout';
import { NavigationButtons } from './NavigationButtons';

// million-ignore
const TopBar = () => {
	const transparentBg = useShowControls().transparentBg;
	const isDragSelecting = useSelector(explorerStore, (s) => s.isDragSelecting);
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
					isDragSelecting && 'pointer-events-none'
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

	const { t } = useLocale();

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
							: 'top-bar-blur border-t border-sidebar-divider bg-sidebar/30 text-ink-faint/60 transition-colors hover:bg-app/50'
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
							className="absolute right-2 rounded p-1 text-ink opacity-0 transition-opacity hover:bg-app-selected group-hover:opacity-100"
						>
							<X />
						</div>
					)}
				</button>
			))}
			<div
				className="flex h-full flex-1 items-center justify-start border-t border-sidebar-divider bg-sidebar/30 px-2"
				data-tauri-drag-region
			>
				<Tooltip keybinds={[keybind.icon, 'T']} label={t('new_tab')}>
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

	useShortcut('newTab', (e) => {
		if (!visible) return;

		e.stopPropagation();

		props.addTab();
	});

	useShortcut('closeTab', (e) => {
		if (!visible) return;

		e.stopPropagation();

		props.removeTab(ctx.tabIndex);
	});

	useShortcut('nextTab', (e) => {
		if (!visible) return;

		e.stopPropagation();

		ctx.setTabIndex(Math.min(ctx.tabIndex + 1, ctx.tabs.length - 1));
	});

	useShortcut('previousTab', (e) => {
		if (!visible) return;

		e.stopPropagation();

		ctx.setTabIndex(Math.max(ctx.tabIndex - 1, 0));
	});
}
