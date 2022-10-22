import { CogIcon, LockClosedIcon } from '@heroicons/react/24/outline';
import { PlusIcon } from '@heroicons/react/24/solid';
import {
	LocationCreateArgs,
	useCurrentLibrary,
	useLibraryMutation,
	useLibraryQuery,
	usePlatform
} from '@sd/client';
import { Button, CategoryHeading, Dropdown, OverlayPanel, cva, tw } from '@sd/ui';
import clsx from 'clsx';
import { CheckCircle, CirclesFour, Planet, ShareNetwork } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import { NavLink, NavLinkProps, useNavigate } from 'react-router-dom';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { Folder } from '../icons/Folder';
import { JobsManager } from '../jobs/JobManager';
import RunningJobsWidget from '../jobs/RunningJobsWidget';
import { MacTrafficLights } from '../os/TrafficLights';

const sidebarItemClass = cva(
	'max-w mb-[2px] rounded px-2 py-1 gap-0.5 flex flex-row flex-grow items-center font-medium truncate text-sm',
	{
		variants: {
			isActive: {
				true: 'bg-sidebar-selected text-ink',
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

const SidebarCategoryHeading = tw(CategoryHeading)`mt-5 mb-1 ml-1`;

function LibraryScopedSection() {
	const platform = usePlatform();
	const { data: locations } = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const { data: tags } = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const { mutate: createLocation } = useLibraryMutation('locations.create');

	return (
		<>
			<div>
				<SidebarCategoryHeading>Locations</SidebarCategoryHeading>
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
							'w-full px-2 py-1.5 mt-1 text-xs font-bold text-center text-ink-faint',
							'rounded border border-dashed border-sidebar-line hover:border-sidebar-selected',
							'cursor-normal transition'
						)}
					>
						Add Location
					</button>
				)}
			</div>
			{tags?.length ? (
				<div>
					<SidebarCategoryHeading>Tags</SidebarCategoryHeading>
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
				</div>
			) : (
				<></>
			)}
		</>
	);
}

export function Sidebar() {
	const navigate = useNavigate();
	const os = useOperatingSystem();
	const { library, libraries, isLoading: isLoadingLibraries, switchLibrary } = useCurrentLibrary();

	// const itemStyles = macOnly(os, 'dark:hover:bg-gray-550 dark:hover:bg-opacity-50');

	return (
		<div
			className={clsx(
				'flex flex-col flex-grow-0 flex-shrink-0 w-44 min-h-full px-2.5 overflow-x-hidden overflow-y-scroll border-r border-sidebar-divider no-scrollbar bg-sidebar',
				macOnly(os, 'bg-opacity-90')
			)}
		>
			<WindowControls />

			<Dropdown.Root
				className="mt-2"
				itemsClassName="bg-app-box border-sidebar-line"
				button={
					<Dropdown.Button
						variant="gray"
						className={clsx(
							`w-full mb-1 mt-1 -mr-0.5 shadow-xs rounded`,
							`bg-sidebar-button border-sidebar-line active:bg-sidebar-button hover:!border-sidebar-selected text-ink`,
							(library === null || isLoadingLibraries) && '!text-ink-faint',
							macOnly(os, '!bg-opacity-80 !border-opacity-40')
						)}
					>
						<span className="truncate">
							{isLoadingLibraries ? 'Loading...' : library ? library.config.name : ' '}
						</span>
					</Dropdown.Button>
				}
				// to support the transparent sidebar on macOS we use slightly adjusted styles
				// itemsClassName={macOnly(os, 'bg-app/60')}
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
					<Dropdown.Item icon={LockClosedIcon} onClick={() => alert('TODO: Not implemented yet!')}>
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

			{library && <RunningJobsWidget />}

			<div className="mt-2 mb-3">
				<NavLink to="/settings/general">
					{({ isActive }) => (
						<Button padding="sm" variant="default" className={clsx('hover:!bg-opacity-20')}>
							<CogIcon className="w-5 h-5" />
						</Button>
					)}
				</NavLink>
				<OverlayPanel
					className="focus:outline-none"
					transformOrigin="bottom left"
					disabled={!library}
					trigger={
						<Button
							padding="sm"
							className={clsx(
								'!outline-none hover:!bg-opacity-20 disabled:opacity-50 disabled:cursor-not-allowed'
							)}
						>
							<CheckCircle className="w-5 h-5" />
						</Button>
					}
				>
					<div className="block w-[500px] h-96">
						<JobsManager />
					</div>
				</OverlayPanel>
			</div>
		</div>
	);
}
