import { CogIcon, LockClosedIcon } from '@heroicons/react/24/outline';
import { PlusIcon } from '@heroicons/react/24/solid';
import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import {
	LocationCreateArgs,
	getDebugState,
	useBridgeQuery,
	useCurrentLibrary,
	useDebugState,
	useLibraryMutation,
	useLibraryQuery,
	usePlatform
} from '@sd/client';
import {
	Button,
	ButtonLink,
	CategoryHeading,
	Dropdown,
	OverlayPanel,
	Switch,
	cva,
	tw
} from '@sd/ui';
import clsx from 'clsx';
import { CheckCircle, CirclesFour, Planet, ShareNetwork } from 'phosphor-react';
import React, { PropsWithChildren } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { Folder } from '../icons/Folder';
import { JobsManager } from '../jobs/JobManager';
import { MacTrafficLights } from '../os/TrafficLights';
import { InputContainer } from '../primitive/InputContainer';

export function Sidebar() {
	const os = useOperatingSystem();
	const { library, libraries, isLoading: isLoadingLibraries, switchLibrary } = useCurrentLibrary();
	const debugState = useDebugState();

	return (
		<div
			className={clsx(
				'flex relative flex-col flex-grow-0 flex-shrink-0 w-44 min-h-full   border-r border-sidebar-divider  bg-sidebar',
				macOnly(os, 'bg-opacity-[0.80]')
			)}
		>
			<WindowControls />

			<div className="flex flex-col px-2.5 flex-grow pt-1 pb-10 overflow-x-hidden overflow-y-scroll no-scrollbar mask-fade-out">
				<Dropdown.Root
					// usually this panel is styled with bg-menu, but as the dark theme sidebar is dark, we need to override it for dark:
					itemsClassName="dark:bg-sidebar-box"
					button={
						<Dropdown.Button
							variant="gray"
							className={clsx(
								`w-full mb-1 mt-1 -mr-0.5`,
								` !bg-sidebar-box !border-sidebar-line/50 active:!border-sidebar-line active:!bg-sidebar-button ui-open:!bg-sidebar-button ui-open:!border-sidebar-line text-ink`,
								(library === null || isLoadingLibraries) && '!text-ink-faint'
								// macOnly(os, '!bg-opacity-80 !border-opacity-40')
							)}
						>
							<span className="truncate">
								{isLoadingLibraries ? 'Loading...' : library ? library.config.name : ' '}
							</span>
						</Dropdown.Button>
					}
				>
					<Dropdown.Section>
						{libraries?.map((lib) => (
							<Dropdown.Item
								selected={lib.uuid === library?.uuid}
								key={lib.uuid}
								onClick={() => switchLibrary(lib.uuid)}
							>
								{lib.config.name}
							</Dropdown.Item>
						))}
					</Dropdown.Section>
					<Dropdown.Section>
						<Dropdown.Item icon={CogIcon} to="settings/library">
							Library Settings
						</Dropdown.Item>
						<CreateLibraryDialog>
							<Dropdown.Item icon={PlusIcon}>Add Library</Dropdown.Item>
						</CreateLibraryDialog>
						<Dropdown.Item
							icon={LockClosedIcon}
							onClick={() => alert('TODO: Not implemented yet!')}
						>
							Lock
						</Dropdown.Item>
					</Dropdown.Section>
				</Dropdown.Root>
				<div className="pt-1">
					<SidebarLink to="/overview">
						<Icon component={Planet} />
						Overview
					</SidebarLink>
					<SidebarLink to="photos">
						<Icon component={ShareNetwork} />
						Nodes
					</SidebarLink>
					<SidebarLink to="content">
						<Icon component={CirclesFour} />
						Spaces
					</SidebarLink>
				</div>
				{library && <LibraryScopedSection />}
				<div className="flex-grow" />
			</div>
			{/* <div className="fixed w-[174px] bottom-[2px] left-[2px] h-20 rounded-[8px] bg-gradient-to-t from-sidebar-box/90 via-sidebar-box/50 to-transparent" /> */}

			<div className="flex flex-col mb-3 px-2.5">
				<div className="flex flex-row">
					<ButtonLink to="/settings/general" size="icon" variant="outline">
						<CogIcon className="w-5 h-5" />
					</ButtonLink>
					<OverlayPanel
						className="focus:outline-none"
						transformOrigin="bottom left"
						disabled={!library}
						trigger={
							<Button
								size="icon"
								variant="outline"
								className="radix-state-open:bg-sidebar-selected/50"
							>
								<CheckCircle className="w-5 h-5" />
							</Button>
						}
					>
						<div className="block w-[430px] h-96">
							<JobsManager />
						</div>
					</OverlayPanel>
				</div>
				{debugState.enabled && <DebugPanel />}
			</div>
		</div>
	);
}

function DebugPanel() {
	const buildInfo = useBridgeQuery(['buildInfo']);
	const nodeState = useBridgeQuery(['nodeState']);
	const debugState = useDebugState();
	const platform = usePlatform();

	return (
		<OverlayPanel
			className="focus:outline-none p-4"
			transformOrigin="bottom left"
			trigger={
				<h1 className="w-full ml-1 mt-1 text-[7pt] text-ink-faint/50">
					v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
				</h1>
			}
		>
			<div className="block w-[430px] h-96">
				<InputContainer
					mini
					title="rspc Logger"
					description="Enable the logger link so you can see what's going on in the browser logs."
				>
					<Switch
						checked={debugState.rspcLogger}
						onClick={() => (getDebugState().rspcLogger = !debugState.rspcLogger)}
					/>
				</InputContainer>
				{platform.openPath && (
					<InputContainer
						mini
						title="Open Data Directory"
						description="Quickly get to your Spacedrive database"
					>
						<div className="mt-2">
							<Button
								size="sm"
								variant="gray"
								onClick={() => {
									if (nodeState?.data?.data_path) platform.openPath!(nodeState?.data?.data_path);
								}}
							>
								Open
							</Button>
						</div>
					</InputContainer>
				)}
				{/* {platform.showDevtools && (
					<InputContainer
						mini
						title="Devtools"
						description="Allow opening browser devtools in a production build"
					>
						<div className="mt-2">
							<Button size="sm" variant="gray" onClick={platform.showDevtools}>
								Show
							</Button>
						</div>
					</InputContainer>
				)} */}
			</div>
		</OverlayPanel>
	);
}

const sidebarItemClass = cva(
	'max-w mb-[2px] rounded px-2 py-1 gap-0.5 flex flex-row flex-grow items-center font-medium truncate text-sm',
	{
		variants: {
			isActive: {
				true: 'bg-sidebar-selected/40 text-ink',
				false: 'text-ink-dull'
			},
			isTransparent: {
				true: 'bg-opacity-90',
				false: ''
			}
		}
	}
);

export const SidebarLink = (props: PropsWithChildren<NavLinkProps>) => {
	const os = useOperatingSystem();
	return (
		<NavLink {...props}>
			{({ isActive }) => (
				<span
					className={clsx(
						sidebarItemClass({ isActive, isTransparent: os === 'macOS' }),
						props.className
					)}
				>
					{props.children}
				</span>
			)}
		</NavLink>
	);
};

const SidebarSection: React.FC<{
	name: string;
	actionArea?: React.ReactNode;
	children: React.ReactNode;
}> = (props) => {
	return (
		<div className="mt-5 group">
			<div className="flex items-center justify-between mb-1">
				<CategoryHeading className="ml-1">{props.name}</CategoryHeading>
				<div className="transition-all duration-300 opacity-0 text-ink-faint group-hover:opacity-30 hover:!opacity-100">
					{props.actionArea}
				</div>
			</div>
			{props.children}
		</div>
	);
};

const SidebarHeadingOptionsButton: React.FC<{ to: string; icon?: React.FC }> = (props) => {
	const Icon = props.icon ?? Ellipsis;
	return (
		<NavLink to={props.to}>
			<Button className="!p-[5px]" variant="outline">
				<Icon className="w-3 h-3" />
			</Button>
		</NavLink>
	);
};

function LibraryScopedSection() {
	const platform = usePlatform();
	const { data: locations } = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const { data: tags } = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const { mutate: createLocation } = useLibraryMutation('locations.create');

	return (
		<>
			<div>
				<SidebarSection
					name="Locations"
					actionArea={
						<>
							{/* <SidebarHeadingOptionsButton to="/settings/locations" icon={CogIcon} /> */}
							<SidebarHeadingOptionsButton to="/settings/locations" />
						</>
					}
				>
					{locations?.map((location) => {
						return (
							<div key={location.id} className="flex flex-row items-center">
								<NavLink
									className="relative w-full group"
									to={{
										pathname: `location/${location.id}`
									}}
								>
									{({ isActive }) => (
										<span className={sidebarItemClass({ isActive })}>
											<div className="-mt-0.5 mr-1 flex-grow-0 flex-shrink-0">
												<Folder size={18} />
											</div>

											<span className="flex-grow flex-shrink-0">{location.name}</span>
										</span>
									)}
								</NavLink>
							</div>
						);
					})}
					{(locations?.length || 0) < 4 && (
						<button
							onClick={() => {
								if (!platform.openFilePickerDialog) {
									// TODO: Support opening locations on web
									alert('Opening a dialogue is not supported on this platform!');
									return;
								}
								platform.openFilePickerDialog().then((result) => {
									// TODO: Pass indexer rules ids to create location
									if (result)
										createLocation({
											path: result as string,
											indexer_rules_ids: []
										} as LocationCreateArgs);
								});
							}}
							className={clsx(
								'w-full px-2 py-1 mt-1 text-xs font-medium text-center',
								'rounded border border-dashed border-sidebar-line hover:border-sidebar-selected',
								'cursor-normal transition text-ink-faint'
							)}
						>
							Add Location
						</button>
					)}
				</SidebarSection>
			</div>
			{!!tags?.length && (
				<SidebarSection
					name="Tags"
					actionArea={<SidebarHeadingOptionsButton to="/settings/tags" />}
				>
					<div className="mt-1 mb-2">
						{tags?.slice(0, 6).map((tag, index) => (
							<SidebarLink key={index} to={`tag/${tag.id}`} className="">
								<div
									className="w-[12px] h-[12px] rounded-full"
									style={{ backgroundColor: tag.color || '#efefef' }}
								/>
								<span className="ml-1.5 text-sm">{tag.name}</span>
							</SidebarLink>
						))}
					</div>
				</SidebarSection>
			)}
		</>
	);
}

const Icon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

// cute little helper to decrease code clutter
const macOnly = (platform: string | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';

function WindowControls() {
	const { platform } = usePlatform();

	const showControls = window.location.search.includes('showControls');
	if (platform === 'tauri' || showControls) {
		return (
			<div data-tauri-drag-region className="flex-shrink-0 h-7">
				{/* We do not provide the onClick handlers for 'MacTrafficLights' because this is only used in demo mode */}
				{showControls && <MacTrafficLights className="z-50 absolute top-[13px] left-[13px]" />}
			</div>
		);
	}

	return null;
}
