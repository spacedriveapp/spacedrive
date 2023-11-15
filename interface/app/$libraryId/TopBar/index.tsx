import { Plus, X } from '@phosphor-icons/react';
import clsx from 'clsx';
import type { Ref } from 'react';
import { useOperatingSystem, useShowControls } from '~/hooks';
import { useTabsContext } from '~/TabsContext';

import { useExplorerStore } from '../Explorer/store';
import { NavigationButtons } from './NavigationButtons';
import SearchBar from './SearchBar';

export const TOP_BAR_HEIGHT = 46;
export const TAB_SWITCHER_HEIGHT = 30;

interface Props {
	leftRef?: Ref<HTMLDivElement>;
	rightRef?: Ref<HTMLDivElement>;
	noSearch?: boolean;
}

const TopBar = (props: Props) => {
	const transparentBg = useShowControls().transparentBg;
	const { isDragging } = useExplorerStore();
	const os = useOperatingSystem();

	return (
		<div
			className={clsx(
				'top-bar-blur absolute inset-x-0 z-50 border-b border-sidebar-divider',
				transparentBg ? 'bg-app/0' : 'bg-app/90'
			)}
		>
			<div
				data-tauri-drag-region={os === 'macOS'}
				style={{ height: TOP_BAR_HEIGHT }}
				className={clsx(
					'flex items-center gap-3.5 overflow-hidden  px-3.5',
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
			<Tabs />
		</div>
	);
};

export default TopBar;

function Tabs() {
	const ctx = useTabsContext();

	if (!ctx || ctx.routers.length < 2) return null;

	return (
		<div
			className="no-scrollbar flex w-full flex-row divide-x divide-sidebar-divider overflow-x-auto bg-black/40 text-ink-dull"
			style={{ height: TAB_SWITCHER_HEIGHT }}
		>
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

								ctx.setRouters((r) => {
									const newRouters = r.filter((_, i) => i !== index);

									if (newRouters.length >= ctx.routerIndex)
										ctx.setRouterIndex(newRouters.length - 1);

									return newRouters;
								});
							}}
							className="absolute right-2 rounded p-1 opacity-0 transition-opacity hover:bg-app-selected group-hover:opacity-100"
						>
							<X />
						</div>
					</button>
				))}
				<button
					onClick={() => {
						const newRouter = ctx.createRouter();
						ctx.setRouters([...ctx.routers, newRouter]);
						ctx.setRouterIndex(ctx.routers.length);
					}}
					className="duration-[50ms] flex flex-row items-center justify-center px-2 transition-colors hover:bg-app/50"
				>
					<Plus weight="bold" size={14} />
				</button>
			</div>
		</div>
	);
}
