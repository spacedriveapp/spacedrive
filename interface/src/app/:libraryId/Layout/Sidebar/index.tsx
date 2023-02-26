import clsx from 'clsx';
import {
	ArchiveBox,
	Broadcast,
	CheckCircle,
	CirclesFour,
	CopySimple,
	Crosshair,
	Eraser,
	FilmStrip,
	Gear,
	MonitorPlay,
	Planet
} from 'phosphor-react';
import { useEffect } from 'react';
import { Link, NavLink } from 'react-router-dom';
import {
	arraysEqual,
	useClientContext,
	useDebugState,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import { Button, ButtonLink, Folder, Loader, Popover, Tooltip } from '@sd/ui';
import { SubtleButton } from '~/components/SubtleButton';
import { MacTrafficLights } from '~/components/TrafficLights';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { OperatingSystem, usePlatform } from '~/util/Platform';
import AddLocationButton from './AddLocationButton';
import DebugPopover from './DebugPopover';
import Icon from './Icon';
import { JobsManager } from './JobManager';
import LibrariesDropdown from './LibrariesDropdown';
import SidebarLink from './Link';
import Section from './Section';

export default () => {
	const os = useOperatingSystem();

	useEffect(() => {
		// Prevent the dropdown button to be auto focused on launch
		// Hacky but it works
		setTimeout(() => {
			if (!document.activeElement || !('blur' in document.activeElement)) return;

			(document.activeElement.blur as () => void)();
		});
	}, []);

	return (
		<div
			className={clsx(
				'border-sidebar-divider bg-sidebar relative flex min-h-full w-44 flex-shrink-0 flex-grow-0 flex-col border-r',
				macOnly(os, 'bg-opacity-[0.75]')
			)}
		>
			<WindowControls />
			<LibrariesDropdown />
			<Contents />
			<Footer />
		</div>
	);
};

const WindowControls = () => {
	const { platform } = usePlatform();
	const os = useOperatingSystem();

	const showControls = window.location.search.includes('showControls');

	if (platform === 'tauri' || showControls) {
		return (
			<div data-tauri-drag-region className={clsx('shrink-0', macOnly(os, 'h-7'))}>
				{/* We do not provide the onClick handlers for 'MacTrafficLights' because this is only used in demo mode */}
				{showControls && <MacTrafficLights className="absolute top-[13px] left-[13px] z-50" />}
			</div>
		);
	}

	return null;
};

const LibrarySection = () => {
	const locations = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();

	return (
		<>
			<Section
				name="Locations"
				actionArea={
					<Link to="settings/locations">
						<SubtleButton />
					</Link>
				}
			>
				{locations.data?.map((location) => {
					const online = onlineLocations?.some((l) => arraysEqual(location.pub_id, l));

					return (
						<SidebarLink
							className="group relative w-full"
							to={`location/${location.id}`}
							key={location.id}
						>
							<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
								<Folder size={18} />
								<div
									className={clsx(
										'absolute right-0 bottom-0.5 h-1.5 w-1.5 rounded-full',
										online ? 'bg-green-500' : 'bg-red-500'
									)}
								/>
							</div>

							<span className="shrink-0 grow">{location.name}</span>
						</SidebarLink>
					);
				})}
				{(locations.data?.length || 0) < 4 && <AddLocationButton />}
			</Section>
			{!!tags.data?.length && (
				<Section
					name="Tags"
					actionArea={
						<NavLink to="settings/tags">
							<SubtleButton />
						</NavLink>
					}
				>
					<div className="mt-1 mb-2">
						{tags.data?.slice(0, 6).map((tag, index) => (
							<SidebarLink key={index} to={`tag/${tag.id}`} className="">
								<div
									className="h-[12px] w-[12px] rounded-full"
									style={{ backgroundColor: tag.color || '#efefef' }}
								/>
								<span className="ml-1.5 text-sm">{tag.name}</span>
							</SidebarLink>
						))}
					</div>
				</Section>
			)}
		</>
	);
};

const Contents = () => {
	const { library } = useClientContext();

	return (
		<div className="no-scrollbar mask-fade-out flex flex-grow flex-col overflow-x-hidden overflow-y-scroll px-2.5 pt-1 pb-10">
			<div className="pt-1">
				<SidebarLink to="overview">
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
				<SidebarLink to="imports">
					<Icon component={ArchiveBox} />
					Imports
				</SidebarLink>
			</div>
			{library && <LibrarySection />}
			<Section name="Tools" actionArea={<SubtleButton />}>
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
			</Section>
			<div className="grow" />
		</div>
	);
};

const IsRunningJob = () => {
	const { data: isRunningJob } = useLibraryQuery(['jobs.isRunning']);

	return isRunningJob ? (
		<Loader className="h-[20px] w-[20px]" />
	) : (
		<CheckCircle className="h-5 w-5" />
	);
};

const Footer = () => {
	const { library } = useClientContext();
	const debugState = useDebugState();

	return (
		<div className="mb-3 flex flex-col px-2.5">
			<div className="flex">
				<ButtonLink
					to="settings/client/general"
					size="icon"
					variant="subtle"
					className="text-ink-faint ring-offset-sidebar"
				>
					<Tooltip label="Settings">
						<Gear className="h-5 w-5" />
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
					<div className="block h-96 w-[430px]">
						<JobsManager />
					</div>
				</Popover>
			</div>
			{debugState.enabled && <DebugPopover />}
		</div>
	);
};

// cute little helper to decrease code clutter
const macOnly = (platform: OperatingSystem | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';
