import { LockClosedIcon, PhotographIcon } from '@heroicons/react/outline';
import { CogIcon, PlusIcon } from '@heroicons/react/solid';
import {
	AppPropsContext,
	useCurrentLibrary,
	useLibraryMutation,
	useLibraryQuery,
	useLibraryStore
} from '@sd/client';
import { Button, Dropdown } from '@sd/ui';
import clsx from 'clsx';
import { CirclesFour, Planet } from 'phosphor-react';
import React, { useContext, useEffect } from 'react';
import { NavLink, NavLinkProps, useNavigate } from 'react-router-dom';

import { Folder } from '../icons/Folder';
import RunningJobsWidget from '../jobs/RunningJobsWidget';
import { MacTrafficLights } from '../os/TrafficLights';
import { DefaultProps } from '../primitive/types';

interface SidebarProps extends DefaultProps {}

export const SidebarLink = (props: NavLinkProps & { children: React.ReactNode }) => (
	<NavLink {...props}>
		{({ isActive }) => (
			<span
				className={clsx(
					'max-w mb-[2px] text-gray-550 dark:text-gray-300 rounded px-2 py-1 flex flex-row flex-grow items-center font-medium text-sm',
					{
						'!bg-primary !text-white hover:bg-primary dark:hover:bg-primary': isActive
					},
					props.className
				)}
			>
				{props.children}
			</span>
		)}
	</NavLink>
);

const Icon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

const Heading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
	<div className="mt-5 mb-1 ml-1 text-xs font-semibold text-gray-300">{children}</div>
);

export const MacWindowControlsSpace: React.FC<{
	children?: React.ReactNode;
}> = (props) => {
	const { children } = props;

	return (
		<div data-tauri-drag-region className="flex-shrink-0 h-7">
			{children}
		</div>
	);
};

export function MacWindowControls() {
	const appProps = useContext(AppPropsContext);

	return (
		<MacWindowControlsSpace>
			<MacTrafficLights
				onClose={appProps?.onClose}
				onFullscreen={appProps?.onFullscreen}
				onMinimize={appProps?.onMinimize}
				className="z-50 absolute top-[13px] left-[13px]"
			/>
		</MacWindowControlsSpace>
	);
}

// cute little helper to decrease code clutter
const macOnly = (platform: string | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';

export const Sidebar: React.FC<SidebarProps> = (props) => {
	const navigate = useNavigate();
	const appProps = useContext(AppPropsContext);
	const { data: locations } = useLibraryQuery(['locations.get']);

	// initialize libraries
	const { init: initLibraries, switchLibrary } = useLibraryStore();
	const { currentLibrary, libraries, currentLibraryUuid } = useCurrentLibrary();

	useEffect(() => {
		if (libraries && !currentLibraryUuid) initLibraries(libraries);
	}, [libraries, currentLibraryUuid]);

	const { mutate: createLocation } = useLibraryMutation('locations.create');

	const { data: tags } = useLibraryQuery(['tags.get']);

	return (
		<div
			className={clsx(
				'flex flex-col flex-grow-0 flex-shrink-0 w-48 min-h-full px-2.5 overflow-x-hidden overflow-y-scroll border-r border-gray-100 no-scrollbar bg-gray-50 dark:bg-gray-850 dark:border-gray-750',
				{
					'dark:!bg-opacity-40': appProps?.platform === 'macOS'
				}
			)}
		>
			{appProps?.platform === 'browser' && window.location.search.includes('showControls') ? (
				<MacWindowControls />
			) : null}
			{appProps?.platform === 'macOS' ? <MacWindowControlsSpace /> : null}

			<Dropdown
				buttonProps={{
					justifyLeft: true,
					className: clsx(
						`flex w-full text-left max-w-full mb-1 mt-1 -mr-0.5 shadow-xs rounded 
          !bg-gray-50 
          border-gray-150 
          hover:!bg-gray-1000 
					
          dark:!bg-gray-500 
					dark:hover:!bg-gray-500

          dark:!border-gray-550
          dark:hover:!border-gray-500
					`,
						appProps?.platform === 'macOS' &&
							'dark:!bg-opacity-40 dark:hover:!bg-opacity-70 dark:!border-[#333949] dark:hover:!border-[#394052]'
					),
					variant: 'gray'
				}}
				// to support the transparent sidebar on macOS we use slightly adjusted styles
				itemsClassName={macOnly(appProps?.platform, 'dark:bg-gray-800	dark:divide-gray-600')}
				itemButtonClassName={macOnly(
					appProps?.platform,
					'dark:hover:bg-gray-550 dark:hover:bg-opacity-50'
				)}
				// this shouldn't default to "My Library", it is only this way for landing demo
				// TODO: implement demo mode for the sidebar and show loading indicator instead of "My Library"
				buttonText={currentLibrary?.config.name || ' '}
				items={[
					libraries?.map((library) => ({
						name: library.config.name,
						selected: library.uuid === currentLibraryUuid,
						onPress: () => switchLibrary(library.uuid)
					})) || [],
					[
						{
							name: 'Library Settings',
							icon: CogIcon,
							onPress: () => navigate('settings/library')
						},
						{
							name: 'Add Library',
							icon: PlusIcon,
							onPress: () => {
								alert('todo');
								// TODO: Show Dialog defined in `LibrariesSettings.tsx`
							}
						},
						{
							name: 'Lock',
							icon: LockClosedIcon,
							onPress: () => {
								alert('todo');
							}
						}
						// { name: 'Hide', icon: EyeOffIcon }
					]
				]}
			/>

			<div className="pt-1">
				<SidebarLink to="/overview">
					<Icon component={Planet} />
					Overview
				</SidebarLink>
				<SidebarLink to="content">
					<Icon component={CirclesFour} />
					Spaces
				</SidebarLink>
				<SidebarLink to="photos">
					<Icon component={PhotographIcon} />
					Photos
				</SidebarLink>
			</div>
			<div>
				<Heading>Locations</Heading>
				{locations?.map((location, index) => {
					return (
						<div key={index} className="flex flex-row items-center">
							<NavLink
								className="relative w-full group"
								to={{
									pathname: `explorer/${location.id}`
								}}
							>
								{({ isActive }) => (
									<span
										className={clsx(
											'max-w mb-[2px] text-gray-550 dark:text-gray-150 rounded px-2 py-1 gap-2 flex flex-row flex-grow items-center  truncate text-sm',
											{
												'!bg-primary !text-white hover:bg-primary dark:hover:bg-primary': isActive
											}
										)}
									>
										<div className="-mt-0.5 flex-grow-0 flex-shrink-0">
											<Folder size={18} className={clsx(!isActive && 'hidden')} white />
											<Folder size={18} className={clsx(isActive && 'hidden')} />
										</div>

										<span className="flex-grow flex-shrink-0">{location.name}</span>
									</span>
								)}
							</NavLink>
						</div>
					);
				})}

				{(locations?.length || 0) < 1 && (
					<button
						onClick={() => {
							appProps?.openDialog({ directory: true }).then((result) => {
								console.log(result);
								if (result) createLocation(result as string);
							});
						}}
						className={clsx(
							'w-full px-2 py-1.5 mt-1 text-xs font-bold text-center text-gray-400 border border-dashed rounded border-transparent cursor-normal border-gray-350 transition',
							appProps?.platform === 'macOS'
								? 'dark:text-gray-450 dark:border-gray-450 hover:dark:border-gray-400 dark:border-opacity-60'
								: 'dark:text-gray-450 dark:border-gray-550 hover:dark:border-gray-500'
						)}
					>
						Add Location
					</button>
				)}
			</div>
			{tags?.length ? (
				<div>
					<Heading>Tags</Heading>
					<div className="mb-2">
						{tags?.slice(0, 6).map((tag, index) => (
							<SidebarLink key={index} to={`tag/${tag.id}`} className="">
								<div
									className="w-[12px] h-[12px] rounded-full"
									style={{ backgroundColor: tag.color || '#efefef' }}
								/>
								<span className="ml-2 text-sm">{tag.name}</span>
							</SidebarLink>
						))}
					</div>
				</div>
			) : (
				<></>
			)}
			<div className="flex-grow" />
			<RunningJobsWidget />
			<div className="mb-2">
				<NavLink to="/settings/general">
					{({ isActive }) => (
						<Button
							noPadding
							variant={isActive ? 'default' : 'default'}
							className={clsx('px-[4px] mb-1')}
						>
							<CogIcon className="w-5 h-5" />
						</Button>
					)}
				</NavLink>
			</div>
		</div>
	);
};
