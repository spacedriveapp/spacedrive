import clsx from 'clsx';
import {
	Broadcast,
	CheckCircle,
	CirclesFour,
	CopySimple,
	Crosshair,
	Eraser,
	FilmStrip,
	Gear,
	Lock,
	MonitorPlay,
	Planet,
	Plus
} from 'phosphor-react';
import React, { PropsWithChildren, useEffect } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import {
	Location,
	LocationCreateArgs,
	arraysEqual,
	getDebugState,
	useBridgeQuery,
	useCurrentLibrary,
	useDebugState,
	useLibraryMutation,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import {
	Button,
	ButtonLink,
	CategoryHeading,
	Dropdown,
	Loader,
	Popover,
	Select,
	SelectOption,
	Switch,
	cva,
	dialogManager,
	tw
} from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';
import AddLocationDialog from '../dialog/AddLocationDialog';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { Folder } from '../icons/Folder';
import { JobsManager } from '../jobs/JobManager';
import { MacTrafficLights } from '../os/TrafficLights';
import { InputContainer } from '../primitive/InputContainer';
import { SubtleButton } from '../primitive/SubtleButton';
import { Tooltip } from '../tooltip/Tooltip';

const SidebarBody = tw.div`flex relative flex-col flex-grow-0 flex-shrink-0 w-44 min-h-full border-r border-sidebar-divider bg-sidebar`;

const SidebarContents = tw.div`flex flex-col px-2.5 flex-grow pt-1 pb-10 overflow-x-hidden overflow-y-scroll no-scrollbar mask-fade-out`;

const SidebarFooter = tw.div`flex flex-col mb-3 px-2.5`;

export function Sidebar() {
	// DO NOT DO LIBRARY QUERIES OR MUTATIONS HERE. This is rendered before a library is set.
	const os = useOperatingSystem();
	const { library, libraries, isLoading: isLoadingLibraries, switchLibrary } = useCurrentLibrary();
	const debugState = useDebugState();

	useEffect(() => {
		// Prevent the dropdown button to be auto focused on launch
		// Hacky but it works
		setTimeout(() => {
			if (!document.activeElement || !('blur' in document.activeElement)) return;

			(document.activeElement.blur as () => void)();
		});
	});

	return (
		<SidebarBody className={macOnly(os, 'bg-opacity-[0.75]')}>
			<WindowControls />
			<Dropdown.Root
				className="mt-2 mx-2.5"
				// we override the sidebar dropdown item's hover styles
				// because the dark style clashes with the sidebar
				itemsClassName="dark:bg-sidebar-box dark:border-sidebar-line mt-1 dark:divide-menu-selected/30 shadow-none"
				button={
					<Dropdown.Button
						variant="gray"
						className={clsx(
							`w-full text-ink `,
							// these classname overrides are messy
							// but they work
							`!bg-sidebar-box !border-sidebar-line/50 active:!border-sidebar-line active:!bg-sidebar-button ui-open:!bg-sidebar-button ui-open:!border-sidebar-line ring-offset-sidebar`,
							(library === null || isLoadingLibraries) && '!text-ink-faint'
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
					<Dropdown.Item
						icon={Plus}
						onClick={() => {
							dialogManager.create((dp) => <CreateLibraryDialog {...dp} />);
						}}
					>
						New Library
					</Dropdown.Item>
					<Dropdown.Item icon={Gear} to="settings/library">
						Manage Library
					</Dropdown.Item>
					<Dropdown.Item icon={Lock} onClick={() => alert('TODO: Not implemented yet!')}>
						Lock
					</Dropdown.Item>
				</Dropdown.Section>
			</Dropdown.Root>
			<SidebarContents>
				<div className="pt-1">
					<SidebarLink to="/overview">
						<Icon component={Planet} />
						Overview
					</SidebarLink>
					<SidebarLink to="spaces">
						<Icon component={CirclesFour} />
						Spaces
					</SidebarLink>
					{/* <SidebarLink to="people">
						<Icon component={UsersThree} />
						People
					</SidebarLink> */}
					<SidebarLink to="media">
						<Icon component={MonitorPlay} />
						Media
					</SidebarLink>
					<SidebarLink to="spacedrop">
						<Icon component={Broadcast} />
						Spacedrop
					</SidebarLink>
				</div>
				{library && <LibraryScopedSection />}
				<SidebarSection name="Tools" actionArea={<SubtleButton />}>
					<SidebarLink to="duplicate-finder">
						<Icon component={CopySimple} />
						Duplicate Finder
					</SidebarLink>
					<SidebarLink to="lost-and-found">
						<Icon component={Crosshair} />
						Find a File
					</SidebarLink>
					<SidebarLink to="cache-cleaner">
						<Icon component={Eraser} />
						Cache Cleaner
					</SidebarLink>
					<SidebarLink to="media-encoder">
						<Icon component={FilmStrip} />
						Media Encoder
					</SidebarLink>
				</SidebarSection>
				<div className="flex-grow" />
			</SidebarContents>
			<SidebarFooter>
				<div className="flex">
					<ButtonLink
						to="/settings/general"
						size="icon"
						variant="subtle"
						className="text-ink-faint ring-offset-sidebar"
					>
						<Tooltip label="Settings">
							<Gear className="w-5 h-5" />
						</Tooltip>
					</ButtonLink>
					<Popover
						trigger={
							<Button
								size="icon"
								variant="subtle"
								className="radix-state-open:bg-sidebar-selected/50 text-ink-faint ring-offset-sidebar"
								disabled={!library}
							>
								{library && (
									<Tooltip label="Recent Jobs">
										<IsRunningJob />
									</Tooltip>
								)}
							</Button>
						}
					>
						<div className="block w-[430px] h-96">
							<JobsManager />
						</div>
					</Popover>
				</div>
				{debugState.enabled && <DebugPanel />}
			</SidebarFooter>
		</SidebarBody>
	);
}

function IsRunningJob() {
	const { data: isRunningJob } = useLibraryQuery(['jobs.isRunning']);

	return isRunningJob ? (
		<Loader className="w-[20px] h-[20px]" />
	) : (
		<CheckCircle className="w-5 h-5" />
	);
}

function DebugPanel() {
	const buildInfo = useBridgeQuery(['buildInfo']);
	const nodeState = useBridgeQuery(['nodeState']);
	const debugState = useDebugState();
	const platform = usePlatform();

	return (
		<Popover
			className="p-4 focus:outline-none"
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
				<InputContainer
					mini
					title="React Query Devtools"
					description="Configure the React Query devtools."
				>
					<Select
						value={debugState.reactQueryDevtools}
						size="sm"
						onChange={(value) => (getDebugState().reactQueryDevtools = value as any)}
					>
						<SelectOption value="disabled">Disabled</SelectOption>
						<SelectOption value="invisible">Invisible</SelectOption>
						<SelectOption value="enabled">Enabled</SelectOption>
					</Select>
				</InputContainer>

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
		</Popover>
	);
}

const sidebarItemClass = cva(
	'max-w mb-[2px] rounded px-2 py-1 gap-0.5 flex flex-row flex-grow items-center font-medium truncate text-sm outline-none ring-offset-sidebar focus:ring-2 focus:ring-accent focus:ring-offset-2',
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
		<NavLink
			{...props}
			className={({ isActive }) =>
				clsx(sidebarItemClass({ isActive, isTransparent: os === 'macOS' }), props.className)
			}
		>
			{props.children}
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

function LibraryScopedSection() {
	const platform = usePlatform();

	const locations = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();

	const createLocation = useLibraryMutation('locations.create');

	return (
		<>
			<div>
				<SidebarSection
					name="Locations"
					actionArea={
						<NavLink to="/settings/locations">
							<SubtleButton />
						</NavLink>
					}
				>
					{locations.data?.map((location) => {
						const online = onlineLocations?.some((l) => arraysEqual(location.pub_id, l));

						return (
							<SidebarLocation location={location} online={online ?? false} key={location.id} />
						);
					})}
					{(locations.data?.length || 0) < 4 && (
						<button
							onClick={() => {
								if (platform.platform === 'web') {
									dialogManager.create((dp) => <AddLocationDialog {...dp} />);
								} else {
									if (!platform.openDirectoryPickerDialog) {
										alert('Opening a dialogue is not supported on this platform!');
										return;
									}
									platform.openDirectoryPickerDialog().then((result) => {
										// TODO: Pass indexer rules ids to create location
										if (result)
											createLocation.mutate({
												path: result as string,
												indexer_rules_ids: []
											} as LocationCreateArgs);
									});
								}
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
			{!!tags.data?.length && (
				<SidebarSection
					name="Tags"
					actionArea={
						<NavLink to="/settings/tags">
							<SubtleButton />
						</NavLink>
					}
				>
					<div className="mt-1 mb-2">
						{tags.data?.slice(0, 6).map((tag, index) => (
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

interface SidebarLocationProps {
	location: Location;
	online: boolean;
}

function SidebarLocation({ location, online }: SidebarLocationProps) {
	return (
		<div className="flex flex-row items-center">
			<SidebarLink
				className="relative w-full group"
				to={{
					pathname: `location/${location.id}`
				}}
			>
				<div className="relative -mt-0.5 mr-1 flex-grow-0 flex-shrink-0">
					<Folder size={18} />
					<div
						className={clsx(
							'absolute w-1.5 h-1.5 right-0 bottom-0.5 rounded-full',
							online ? 'bg-green-500' : 'bg-red-500'
						)}
					/>
				</div>

				<span className="flex-grow flex-shrink-0">{location.name}</span>
			</SidebarLink>
		</div>
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
