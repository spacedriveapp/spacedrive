import { Plus, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useLayoutEffect, useRef } from 'react';
import useResizeObserver from 'use-resize-observer';
import { useSelector } from '@sd/client';
import { Tooltip } from '@sd/ui';
import {
	useKeyMatcher,
	useLocale,
	useOperatingSystem,
	useShortcut,
	useShowControls,
	useWindowState
} from '~/hooks';
import { useTabsContext } from '~/TabsContext';

import { explorerStore } from '../Explorer/store';
import { useLayoutStore } from '../Layout/store';
import { useTopBarContext } from './Context';
import { NavigationButtons } from './NavigationButtons';

// million-ignore
const TopBar = () => {
	const transparentBg = useShowControls().transparentBg;
	const isDragSelecting = useSelector(explorerStore, (s) => s.isDragSelecting);

	const ref = useRef<HTMLDivElement>(null);

	const tabs = useTabsContext();
	const ctx = useTopBarContext();

	const windowState = useWindowState();
	const platform = useOperatingSystem();

	const layoutStore = useLayoutStore();

	useResizeObserver({
		ref,
		box: 'border-box',
		onResize(bounds) {
			if (bounds.height === undefined) return;
			ctx.setTopBarHeight(bounds.height);
		}
	});

	//prevent default search from opening from edge webview
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === 'f' && e.ctrlKey) {
				e.preventDefault();
			}
		};
		document.body.addEventListener('keydown', handleKeyDown);
		return () => {
			document.body.removeEventListener('keydown', handleKeyDown);
		};
	}, []);

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
					isDragSelecting && 'pointer-events-none',
					platform === 'macOS' &&
						!windowState.isFullScreen &&
						layoutStore.sidebar.collapsed &&
						'pl-20'
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
						'group relative flex h-full min-w-40 shrink-0 flex-row items-center justify-center px-8 text-center duration-[50ms]',
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
						className="flex flex-row items-center justify-center rounded p-1.5 transition-colors duration-[50ms] hover:bg-app/80"
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

	useShortcut('newTab', (e) => {
		e.stopPropagation();
		if (e.shiftKey) return; //to prevent colliding with other shortcuts
		props.addTab();
	});

	useShortcut('duplicateTab', (e) => {
		e.stopPropagation();
		ctx.duplicateTab();
	});

	useShortcut('closeTab', (e) => {
		e.stopPropagation();
		props.removeTab(ctx.tabIndex);
	});

	useShortcut('nextTab', (e) => {
		e.stopPropagation();
		ctx.setTabIndex(Math.min(ctx.tabIndex + 1, ctx.tabs.length - 1));
	});

	useShortcut('previousTab', (e) => {
		e.stopPropagation();
		ctx.setTabIndex(Math.max(ctx.tabIndex - 1, 0));
	});
}
