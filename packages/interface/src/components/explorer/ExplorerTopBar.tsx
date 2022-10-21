import { ChevronLeftIcon, ChevronRightIcon, TagIcon } from '@heroicons/react/24/outline';
import { KeyIcon as KeyIconSolid, TagIcon as TagIconSolid } from '@heroicons/react/24/solid';
import {
	OperatingSystem,
	getExplorerStore,
	useExplorerStore,
	useLibraryMutation
} from '@sd/client';
import { Dropdown, OverlayPanel } from '@sd/ui';
import clsx from 'clsx';
import {
	Aperture,
	ArrowsClockwise,
	Cloud,
	FilmStrip,
	IconProps,
	Image,
	Key,
	List,
	MonitorPlay,
	Rows,
	SidebarSimple,
	SquaresFour
} from 'phosphor-react';
import { DetailedHTMLProps, HTMLAttributes, forwardRef, useEffect, useRef } from 'react';
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
	icon: React.ComponentType<IconProps>;
	group?: boolean;
	active?: boolean;
	left?: boolean;
	right?: boolean;
	className?: string;
	onClick?: () => void;
}

const TopBarButton = forwardRef<HTMLButtonElement, TopBarButtonProps>(
	({ icon: Icon, left, right, group, active, className, ...props }, ref) => {
		return (
			<button
				{...props}
				ref={ref}
				className={clsx(
					'mr-[1px] flex py-0.5 px-0.5 text-md font-medium rounded-md open:bg-selected transition-colors duration-100 outline-none !cursor-normal',
					{
						'rounded-r-none rounded-l-none': group && !left && !right,
						'rounded-r-none': group && left,
						'rounded-l-none': group && right
					},
					className
				)}
			>
				<Icon weight={'regular'} className="m-0.5 w-5 h-5 text-ink-dull" />
			</button>
		);
	}
);

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
			<input
				ref={(el) => {
					ref(el);

					if (typeof forwardedRef === 'function') forwardedRef(el);
					else if (forwardedRef) forwardedRef.current = el;
				}}
				placeholder="Search"
				className="peer w-32 h-[30px] focus:w-52 text-sm p-3 rounded-lg outline-none focus:ring-2 border shadow  transition-all bg-app-input border-app-border"
				{...searchField}
			/>

			<div
				className={clsx(
					'space-x-1 absolute top-[1px] right-1 peer-focus:invisible pointer-events-none',
					isDirty && 'hidden'
				)}
			>
				{platform === 'browser' ? (
					<Shortcut chars="/" aria-label={'Press slash to focus search bar'} />
				) : os === 'macOS' ? (
					<Shortcut chars="⌘F" aria-label={'Press Command-F to focus search bar'} />
				) : (
					<Shortcut chars="CTRL+F" aria-label={'Press CTRL-F to focus search bar'} />
				)}
				{/* <Shortcut chars="S" /> */}
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
	const { mutate: generateThumbsForLocation } = useLibraryMutation(
		'jobs.generateThumbsForLocation',
		{
			onMutate: (data) => {
				// console.log('GenerateThumbsForLocation', data);
			}
		}
	);

	const { mutate: identifyUniqueFiles } = useLibraryMutation('jobs.identifyUniqueFiles', {
		onMutate: (data) => {
			// console.log('IdentifyUniqueFiles', data);
		},
		onError: (error) => {
			console.error('IdentifyUniqueFiles', error);
		}
	});

	const { mutate: objectValidator } = useLibraryMutation('jobs.objectValidator', {
		onMutate: (data) => {
			// console.log('ObjectValidator', data);
		}
	});

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
				// Backdrop blur was removed
				// but the explorer still resides under the top bar
				// in case you wanna turn it back on
				// honestly its just work to revert
				className={clsx(
					'flex h-[2.95rem] -mt-0.5 max-w z-10 pl-3 flex-shrink-0 items-center border-transparent border-b app-background overflow-hidden rounded-tl-md transition-[background-color] transition-[border-color] duration-250 ease-out',
					props.showSeparator && 'top-bar-blur'
				)}
			>
				<div className="flex">
					<Tooltip label="Navigate back">
						<TopBarButton icon={ChevronLeftIcon} onClick={() => navigate(-1)} />
					</Tooltip>
					<Tooltip label="Navigate forward">
						<TopBarButton icon={ChevronRightIcon} onClick={() => navigate(1)} />
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
								group
								left
								active={store.layoutMode === 'grid'}
								icon={SquaresFour}
								onClick={() => (getExplorerStore().layoutMode = 'grid')}
							/>
						</Tooltip>
						<Tooltip label="List view">
							<TopBarButton
								group
								active={store.layoutMode === 'list'}
								icon={Rows}
								onClick={() => (getExplorerStore().layoutMode = 'list')}
							/>
						</Tooltip>

						<Tooltip label="Media view">
							<TopBarButton
								group
								right
								active={store.layoutMode === 'media'}
								icon={MonitorPlay}
								onClick={() => (getExplorerStore().layoutMode = 'media')}
							/>
						</Tooltip>
					</div>

					<SearchBar ref={searchRef} />

					<div className="flex mx-8 space-x-2">
						<OverlayPanel
							className="focus:outline-none"
							trigger={
								// <Tooltip label="Major Key Alert">
								<TopBarButton icon={Key} />
								// </Tooltip>
							}
						>
							<div className="block w-[350px]">
								<KeyManager />
							</div>
						</OverlayPanel>
						<Tooltip label="Tag Assign Mode">
							<TopBarButton
								onClick={() => (getExplorerStore().tagAssignMode = !store.tagAssignMode)}
								active={store.tagAssignMode}
								icon={store.tagAssignMode ? TagIconSolid : TagIcon}
							/>
						</Tooltip>
						<Tooltip label="Refresh">
							<TopBarButton icon={ArrowsClockwise} />
						</Tooltip>
					</div>
				</div>
				<div className="flex mr-3 space-x-2">
					<OverlayPanel
						className="focus:outline-none"
						trigger={
							// <Tooltip label="Major Key Alert">
							<TopBarButton icon={List} className="my-2" />
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
						icon={SidebarSimple}
					/>
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
