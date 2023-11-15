import { Plus, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useLayoutEffect, useRef, type Ref } from 'react';
import { useKey } from 'rooks';
import useResizeObserver from 'use-resize-observer';
import { useOperatingSystem, useShowControls } from '~/hooks';
import { useTabsContext } from '~/TabsContext';

import { useExplorerStore } from '../Explorer/store';
import { useTopBarContext } from './Layout';
import { NavigationButtons } from './NavigationButtons';
import SearchBar from './SearchBar';

interface Props {
	leftRef?: Ref<HTMLDivElement>;
	rightRef?: Ref<HTMLDivElement>;
	noSearch?: boolean;
}

const TopBar = (props: Props) => {
	const transparentBg = useShowControls().transparentBg;
	const { isDragging } = useExplorerStore();
	const os = useOperatingSystem();

	const ref = useRef<HTMLDivElement>(null);

	const topBar = useTopBarContext();

	useResizeObserver({
		ref,
		box: 'border-box',
		onResize(bounds) {
			if (bounds.height === undefined) return;
			topBar.setTopBarHeight(bounds.height);
		}
	});

	// this is crucial to make sure that the first browser paint takes into account the proper top bar height.
	// resize observer doesn't run early enough to cause react to rerender before the first browser paint
	useLayoutEffect(() => {
		const height = ref.current!.getBoundingClientRect().height;
		topBar.setTopBarHeight(height);
	}, []);

	const tabs = useTabsContext();

	return (
		<div
			ref={ref}
			className={clsx(
				'top-bar-blur absolute inset-x-0 z-50 border-b border-sidebar-divider',
				transparentBg ? 'bg-app/0' : 'bg-app/90'
			)}
		>
			<div
				data-tauri-drag-region={os === 'macOS'}
				className={clsx(
					'flex h-12 items-center gap-3.5 overflow-hidden px-3.5',
					'duration-250 transition-[background-color,border-color] ease-out',
					isDragging && 'pointer-events-none'
				)}
			>
				<div
					data-tauri-drag-region={os === 'macOS'}
					className="flex flex-1 items-center gap-3.5 overflow-hidden"
				>
					<NavigationButtons />
					<div ref={props.leftRef} className="overflow-hidden" />
				</div>

				{!props.noSearch && <SearchBar />}

				<div ref={props.rightRef} className={clsx(!props.noSearch && 'flex-1')} />
			</div>
			{tabs && <Tabs />}
		</div>
	);
};

export default TopBar;

function Tabs() {
	const ctx = useTabsContext()!;

	function addTab() {
		const newRouter = ctx.createRouter();
		ctx.setRouters([...ctx.routers, newRouter]);
		ctx.setRouterIndex(ctx.routers.length);
	}

	function removeTab(index: number) {
		if (ctx.routers.length === 1) return;

		ctx.setRouters((r) => {
			const newRouters = r.filter((_, i) => i !== index);

			if (newRouters.length >= ctx.routerIndex) ctx.setRouterIndex(newRouters.length - 1);

			return newRouters;
		});
	}

	const os = useOperatingSystem();

	// these keybinds aren't part of the regular shortcuts system as they're desktop-only
	useKey(['t'], (e) => {
		if ((os === 'macOS' && !e.metaKey) || (os !== 'macOS' && !e.ctrlKey)) return;

		e.stopPropagation();

		addTab();
	});

	useKey(['w'], (e) => {
		if ((os === 'macOS' && !e.metaKey) || (os !== 'macOS' && !e.ctrlKey)) return;

		e.stopPropagation();

		removeTab(ctx.routerIndex);
	});

	useKey(['ArrowLeft', 'ArrowRight'], (e) => {
		// TODO: figure out non-macos keybind
		if ((os === 'macOS' && !(e.metaKey && e.altKey)) || os !== 'macOS') return;

		e.stopPropagation();

		let delta = e.key === 'ArrowLeft' ? -1 : 1;

		ctx.setRouterIndex(Math.min(Math.max(0, ctx.routerIndex + delta), ctx.routers.length - 1));
	});

	if (ctx.routers.length < 2) return null;

	return (
		<div className="no-scrollbar flex h-8 w-full flex-row divide-x divide-sidebar-divider overflow-x-auto bg-black/40 text-ink-dull">
			<div className="no-scrollbar flex w-full flex-row divide-x divide-sidebar-divider overflow-x-auto">
				{ctx.routers.map((_, index) => (
					<button
						onClick={() => ctx.setRouterIndex(index)}
						className={clsx(
							'duration-[50ms] group relative flex h-full flex-1 flex-row items-center justify-center text-center text-sm',
							ctx.routerIndex === index
								? 'bg-app text-ink'
								: 'transition-colors hover:bg-app/50'
						)}
						key={index}
					>
						Tab {index + 1}
						<div
							onClick={(e) => {
								e.stopPropagation();
								removeTab(index);
							}}
							className="absolute right-2 rounded p-1 opacity-0 transition-opacity hover:bg-app-selected group-hover:opacity-100"
						>
							<X />
						</div>
					</button>
				))}
				<button
					onClick={() => {}}
					className="duration-[50ms] flex flex-row items-center justify-center px-2 transition-colors hover:bg-app/50"
				>
					<Plus weight="bold" size={14} />
				</button>
			</div>
		</div>
	);
}
