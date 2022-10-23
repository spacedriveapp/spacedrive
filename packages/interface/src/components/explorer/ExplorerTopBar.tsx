import { ChevronLeftIcon, ChevronRightIcon, TagIcon } from '@heroicons/react/24/outline';
import { TagIcon as TagIconSolid } from '@heroicons/react/24/solid';
import { getExplorerStore, useExplorerStore, useLibraryMutation } from '@sd/client';
import { Button, Input, OverlayPanel, cva, tw } from '@sd/ui';
import clsx from 'clsx';
import {
	ArrowsClockwise,
	IconProps,
	Key,
	List,
	MonitorPlay,
	Rows,
	SidebarSimple,
	SquaresFour
} from 'phosphor-react';
import { forwardRef, useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import { KeybindEvent } from '../../util/keybind';
import { KeyManager } from '../key/KeyManager';
import { Shortcut } from '../primitive/Shortcut';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';
import { ExplorerOptionsPanel } from './ExplorerOptionsPanel';

export interface TopBarButtonProps {
	children: React.ReactNode;
	rounding?: 'none' | 'left' | 'right' | 'both';
	active?: boolean;
	className?: string;
	onClick?: () => void;
}

// export const TopBarIcon = (icon: any) => tw(icon)`m-0.5 w-5 h-5 text-ink-dull`;

const topBarButtonStyle = cva(
	'border-none text-ink-dull hover:text-ink mr-[1px] flex py-0.5 px-0.5 text-md font-medium transition-colors duration-100 outline-none hover:bg-app-selected radix-state-open:bg-app-selected',
	{
		variants: {
			active: {
				true: 'bg-app-selected',
				false: 'bg-transparent'
			},
			rounding: {
				none: 'rounded-none',
				left: 'rounded-l-md rounded-r-none',
				right: 'rounded-r-md rounded-l-none',
				both: 'rounded-md'
			}
		},
		defaultVariants: {
			active: false,
			rounding: 'both'
		}
	}
);

const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';

const TopBarButton = forwardRef<HTMLButtonElement, TopBarButtonProps>((props, ref) => {
	return (
		<Button {...props} ref={ref} className={clsx(topBarButtonStyle(props), props.className)}>
			{props.children}
		</Button>
	);
});

const SearchBar = forwardRef<HTMLInputElement, DefaultProps>((props, forwardedRef) => {
	const {
		register,
		handleSubmit,
		reset,
		formState: { isDirty, dirtyFields }
	} = useForm();

	const { ref, ...searchField } = register('searchField', {
		onBlur: (e) => {
			// if there's no text in the search bar, don't mark it as dirty so the key hint shows
			if (!dirtyFields.searchField) reset();
		}
	});

	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	return (
		<form onSubmit={handleSubmit(() => null)} className="relative flex h-7">
			<Input
				ref={(el) => {
					ref(el);

					if (typeof forwardedRef === 'function') forwardedRef(el);
					else if (forwardedRef) forwardedRef.current = el;
				}}
				placeholder="Search"
				className="w-32 transition-all focus:w-52"
				{...searchField}
			/>

			<div
				className={clsx(
					'space-x-1 absolute top-[1px] right-1 peer-focus:invisible pointer-events-none'
				)}
			>
				{platform === 'browser' ? (
					<Shortcut chars="⌘F" aria-label={'Press Command-F to focus search bar'} />
				) : os === 'macOS' ? (
					<Shortcut chars="⌘F" aria-label={'Press Command-F to focus search bar'} />
				) : (
					<Shortcut chars="CTRL+F" aria-label={'Press CTRL-F to focus search bar'} />
				)}
			</div>
		</form>
	);
});

export type TopBarProps = DefaultProps & {
	showSeparator?: boolean;
};

export const TopBar: React.FC<TopBarProps> = (props) => {
	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	const store = useExplorerStore();

	// const { mutate: generateThumbsForLocation } = useLibraryMutation(
	// 	'jobs.generateThumbsForLocation'
	// );
	// const { mutate: identifyUniqueFiles } = useLibraryMutation('jobs.identifyUniqueFiles');
	// const { mutate: objectValidator } = useLibraryMutation('jobs.objectValidator');

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
					'flex h-[2.95rem] -mt-0.5 max-w z-10 pl-3 flex-shrink-0 items-center border-transparent border-b bg-app overflow-hidden transition-[background-color] transition-[border-color] duration-250 ease-out',
					props.showSeparator && 'top-bar-blur !bg-app/90'
				)}
			>
				<div className="flex">
					<Tooltip label="Navigate back">
						<TopBarButton onClick={() => navigate(-1)}>
							<ChevronLeftIcon className={TOP_BAR_ICON_STYLE} />
						</TopBarButton>
					</Tooltip>
					<Tooltip label="Navigate forward">
						<TopBarButton onClick={() => navigate(1)}>
							<ChevronRightIcon className={TOP_BAR_ICON_STYLE} />
						</TopBarButton>
					</Tooltip>
				</div>

				{/* <div className="flex mx-8 space-x-[1px]">
          <TopBarButton active group left icon={List} />
          <TopBarButton group icon={Columns} />
          <TopBarButton group right icon={SquaresFour} />
        </div> */}

				<div data-tauri-drag-region className="flex flex-row justify-center flex-grow">
					<div className="flex mx-8">
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
								active={store.layoutMode === 'list'}
								onClick={() => (getExplorerStore().layoutMode = 'list')}
							>
								<Rows className={TOP_BAR_ICON_STYLE} />
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

					<div className="flex mx-8 space-x-2">
						<OverlayPanel
							className="focus:outline-none"
							trigger={
								// <Tooltip label="Major Key Alert">
								<TopBarButton>
									<Key className={TOP_BAR_ICON_STYLE} />
								</TopBarButton>
								// </Tooltip>
							}
						>
							<div className="block w-[350px]">
								<KeyManager className={TOP_BAR_ICON_STYLE} />
							</div>
						</OverlayPanel>
						<Tooltip label="Tag Assign Mode">
							<TopBarButton
								onClick={() => (getExplorerStore().tagAssignMode = !store.tagAssignMode)}
								active={store.tagAssignMode}
							>
								{store.tagAssignMode ? (
									<TagIconSolid className={TOP_BAR_ICON_STYLE} />
								) : (
									<TagIcon className={TOP_BAR_ICON_STYLE} />
								)}
							</TopBarButton>
						</Tooltip>
						<Tooltip label="Refresh">
							<TopBarButton>
								<ArrowsClockwise className={TOP_BAR_ICON_STYLE} />
							</TopBarButton>
						</Tooltip>
					</div>
				</div>
				<div className="flex mr-3 space-x-2">
					<OverlayPanel
						className="focus:outline-none"
						trigger={
							// <Tooltip label="Major Key Alert">
							<TopBarButton className="my-2">
								<List className={TOP_BAR_ICON_STYLE} />
							</TopBarButton>
							// </Tooltip>
						}
					>
						<div className="block w-[250px] ">
							<ExplorerOptionsPanel />
						</div>
					</OverlayPanel>
					<TopBarButton
						active={store.showInspector}
						onClick={() => (getExplorerStore().showInspector = !store.showInspector)}
						className="my-2"
					>
						{store.showInspector ? (
							<SidebarSimple className={TOP_BAR_ICON_STYLE} />
						) : (
							<SidebarSimple className={TOP_BAR_ICON_STYLE} />
						)}
					</TopBarButton>
					{/* <Dropdown
						// className="absolute block h-6 w-44 top-2 right-4"
						align="right"
						items={[
							[
								{
									name: 'Generate Thumbs',
									icon: ArrowsClockwise,
									onPress: () =>
										store.locationId &&
										generateThumbsForLocation({ id: store.locationId, path: '' })
								},
								{
									name: 'Identify Unique',
									icon: ArrowsClockwise,
									onPress: () =>
										store.locationId && identifyUniqueFiles({ id: store.locationId, path: '' })
								},
								{
									name: 'Validate Objects',
									icon: ArrowsClockwise,
									onPress: () =>
										store.locationId && objectValidator({ id: store.locationId, path: '' })
								}
							]
						]}
						buttonComponent={<TopBarButton icon={List} />}
					/> */}
				</div>
			</div>
		</>
	);
};
