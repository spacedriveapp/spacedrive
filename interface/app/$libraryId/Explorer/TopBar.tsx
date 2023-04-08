import clsx from 'clsx';
import {
	ArrowsClockwise,
	CaretLeft,
	CaretRight,
	Columns,
	Key,
	MonitorPlay,
	Rows,
	SidebarSimple,
	SlidersHorizontal,
	SquaresFour,
	Tag
} from 'phosphor-react';
import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { Popover, Tooltip } from '@sd/ui';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { KeybindEvent } from '~/util/keybind';
import { KeyManager } from '../KeyManager';
import OptionsPanel from './OptionsPanel';
import SearchBar from './SearchBar';
import TopBarButton from './TopBarButton';

export type TopBarProps = {
	showSeparator?: boolean;
};

export default (props: TopBarProps) => {
	const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';
	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);
	const store = useExplorerStore();
	const navigate = useNavigate();

	//create function to focus on search box when cmd+k is pressed
	const searchRef = useRef<HTMLInputElement>(null);

	const focusSearchBar = (bar: HTMLInputElement, e?: Event): boolean => {
		bar.focus();

		e?.preventDefault();
		return false;
	};

	useEffect(() => {
		const searchBar = searchRef.current;

		if (searchBar === null || !searchBar) return;

		const handleKeybindAction = (e: KeybindEvent) => {
			if (e.detail.action === 'open_search') {
				return focusSearchBar(searchBar, e);
			}
		};

		const handleDOMKeydown = (e: KeyboardEvent) => {
			if (e.target === searchBar && e.key === 'Escape') {
				(e.target as HTMLInputElement).blur();
				e.preventDefault();
				return;
			}

			const isBrowser = platform === 'browser';
			// use cmd on macOS and ctrl on Windows
			const hasModifier = os === 'macOS' ? e.metaKey : e.ctrlKey;

			if (
				// allow slash on all platforms
				(e.key === '/' &&
					!(document.activeElement instanceof HTMLInputElement) &&
					!(document.activeElement instanceof HTMLTextAreaElement)) ||
				// only do the cmd-f keybind check on browser to allow for native keybind functionality
				// this is particularly useful for power-user niche use cases,
				// like how macOS lets you redefine keybinds for apps
				(isBrowser && hasModifier && e.key === 'f')
			) {
				document.dispatchEvent(new KeybindEvent('open_search'));
				e.preventDefault();
				return;
			}
		};

		document.addEventListener('keydown', handleDOMKeydown);
		document.addEventListener('keybindexec', handleKeybindAction);

		return () => {
			document.removeEventListener('keydown', handleDOMKeydown);
			document.removeEventListener('keybindexec', handleKeybindAction);
		};
	}, [os, platform]);

	return (
		<>
			<div
				data-tauri-drag-region
				className={clsx(
					'max-w duration-250 z-20 flex h-[46px] shrink-0 items-center overflow-hidden border-b border-transparent bg-app pl-3 transition-[background-color] transition-[border-color] ease-out',
					props.showSeparator && 'top-bar-blur !bg-app/90'
				)}
			>
				<div className="flex">
					<Tooltip label="Navigate back">
						<TopBarButton onClick={() => navigate(-1)}>
							<CaretLeft weight="bold" className={TOP_BAR_ICON_STYLE} />
						</TopBarButton>
					</Tooltip>
					<Tooltip label="Navigate forward">
						<TopBarButton onClick={() => navigate(1)}>
							<CaretRight weight="bold" className={TOP_BAR_ICON_STYLE} />
						</TopBarButton>
					</Tooltip>
				</div>

				<div data-tauri-drag-region className="flex grow flex-row justify-center">
					<div className="mx-8 flex">
						<Tooltip label="Grid view">
							<TopBarButton
								rounding="left"
								active={store.layoutMode === 'grid'}
								onClick={() => (getExplorerStore().layoutMode = 'grid')}
							>
								<SquaresFour className={TOP_BAR_ICON_STYLE} />
							</TopBarButton>
						</Tooltip>
						<Tooltip label="List view">
							<TopBarButton
								rounding="none"
								active={store.layoutMode === 'rows'}
								onClick={() => (getExplorerStore().layoutMode = 'rows')}
							>
								<Rows className={TOP_BAR_ICON_STYLE} />
							</TopBarButton>
						</Tooltip>
						<Tooltip label="Columns view">
							<TopBarButton
								rounding="none"
								active={store.layoutMode === 'columns'}
								onClick={() => (getExplorerStore().layoutMode = 'columns')}
							>
								<Columns className={TOP_BAR_ICON_STYLE} />
							</TopBarButton>
						</Tooltip>
						<Tooltip label="Media view">
							<TopBarButton
								rounding="right"
								active={store.layoutMode === 'media'}
								onClick={() => (getExplorerStore().layoutMode = 'media')}
							>
								<MonitorPlay className={TOP_BAR_ICON_STYLE} />
							</TopBarButton>
						</Tooltip>
					</div>

					<SearchBar ref={searchRef} />

					<div className="mx-8 flex space-x-2">
						<Tooltip label="Key Manager">
							<Popover
								className="focus:outline-none"
								trigger={
									<TopBarButton>
										<Key className={TOP_BAR_ICON_STYLE} />
									</TopBarButton>
								}
							>
								<div className="block w-[350px]">
									<KeyManager />
								</div>
							</Popover>
						</Tooltip>
						<Tooltip label="Tag Assign Mode">
							<TopBarButton
								onClick={() => (getExplorerStore().tagAssignMode = !store.tagAssignMode)}
								active={store.tagAssignMode}
							>
								<Tag
									weight={store.tagAssignMode ? 'fill' : 'regular'}
									className={TOP_BAR_ICON_STYLE}
								/>
							</TopBarButton>
						</Tooltip>
						<Tooltip label="Regenerate thumbs (temp)">
							<TopBarButton>
								<ArrowsClockwise className={TOP_BAR_ICON_STYLE} />
							</TopBarButton>
						</Tooltip>
					</div>
				</div>
				<div className="mr-3 flex space-x-2">
					<Tooltip label="Explorer display options" position="left">
						<Popover
							className="focus:outline-none"
							trigger={
								<TopBarButton className="my-2">
									<SlidersHorizontal className={TOP_BAR_ICON_STYLE} />
								</TopBarButton>
							}
						>
							<div className="block w-[250px] ">
								<OptionsPanel />
							</div>
						</Popover>
					</Tooltip>

					<Tooltip
						label={store.showInspector ? 'Hide Inspector' : 'Show Inspector'}
						position="left"
					>
						<TopBarButton
							active={store.showInspector}
							onClick={() => (getExplorerStore().showInspector = !store.showInspector)}
							className="my-2"
						>
							<SidebarSimple
								weight={store.showInspector ? 'fill' : 'regular'}
								className={clsx(TOP_BAR_ICON_STYLE, 'scale-x-[-1]')}
							/>
						</TopBarButton>
					</Tooltip>
				</div>
			</div>
		</>
	);
};
