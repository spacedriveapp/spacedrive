import { CaretDown } from '@phosphor-icons/react';
import { keepPreviousData } from '@tanstack/react-query';
import clsx from 'clsx';
import { useState } from 'react';
import { Link, useMatch } from 'react-router-dom';
import {
	arraysEqual,
	Device,
	Location as LocationType,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import { useExplorerDroppable } from '~/app/$libraryId/Explorer/useExplorerDroppable';
import { useExplorerSearchParams } from '~/app/$libraryId/Explorer/util';
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { Icon, SubtleButton } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import { SeeMore } from '../../SidebarLayout/SeeMore';
import { ContextMenu } from './ContextMenu';

export default function Locations() {
	const locationsQuery = useLibraryQuery(['locations.list'], {
		placeholderData: keepPreviousData
	});
	const locations = locationsQuery.data;
	const onlineLocations = useOnlineLocations();

	const { data: devices } = useLibraryQuery(['devices.list'], {
		placeholderData: keepPreviousData
	});

	const { t } = useLocale();

	// Group locations by device
	const locationsByDevice = locations?.reduce(
		(acc, location) => {
			const deviceId = location.device_id;
			if (!deviceId) return acc;

			if (!acc[deviceId]) {
				acc[deviceId] = [];
			}
			acc[deviceId].push(location);
			return acc;
		},
		{} as Record<number, LocationType[]>
	);

	return (
		<Section
			name={t('locations')}
			actionArea={
				<Link to="settings/library/locations">
					<SubtleButton />
				</Link>
			}
		>
			<SeeMore limit={10}>
				{devices?.map((device) => (
					<DeviceLocations
						key={device.id}
						device={device}
						locations={locationsByDevice?.[device.id] || []}
						onlineLocations={onlineLocations}
					/>
				))}
			</SeeMore>
			<AddLocationButton className="mt-1" />
		</Section>
	);
}

const DeviceLocations = ({
	device,
	locations,
	onlineLocations
}: {
	device: Device;
	locations: LocationType[];
	onlineLocations: number[][];
}) => {
	const [isExpanded, setIsExpanded] = useState(true);

	if (locations.length === 0) return null;

	return (
		<div className="space-y-1">
			<button
				onClick={() => setIsExpanded(!isExpanded)}
				className="flex w-full items-center gap-1 rounded px-2 py-1 hover:bg-app-hover/40"
			>
				{/* <CaretDown
					weight="bold"
					className={clsx('shrink-0 opacity-50', isExpanded ? 'rotate-180' : 'rotate-0')}
				/> */}
				<Icon
					name={
						device.hardware_model
							? hardwareModelToIcon(device.hardware_model)
							: 'Laptop'
					}
					size={18}
					className="shrink-0"
				/>
				<span className="truncate text-sm font-medium text-ink-dull">{device.name}</span>
			</button>
			{isExpanded && (
				<div className="ml-2 space-y-0.5">
					{locations.map((location) => (
						<Location
							key={location.id}
							location={location}
							online={onlineLocations.some((l) => arraysEqual(location.pub_id, l))}
						/>
					))}
				</div>
			)}
		</div>
	);
};

const Location = ({ location, online }: { location: LocationType; online: boolean }) => {
	const locationId = useMatch('/:libraryId/location/:locationId')?.params.locationId;
	const [{ path }] = useExplorerSearchParams();

	const { isDroppable, className, setDroppableRef } = useExplorerDroppable({
		id: `sidebar-location-${location.id}`,
		allow: ['Path', 'NonIndexedPath', 'Object'],
		data: { type: 'location', path: '/', data: location },
		disabled: Number(locationId) === location.id && !path,
		navigateTo: `location/${location.id}`
	});

	return (
		<ContextMenu locationId={location.id}>
			<SidebarLink
				ref={setDroppableRef}
				to={`location/${location.id}`}
				className={clsx(
					'border radix-state-open:border-accent',
					isDroppable ? 'border-accent' : 'border-transparent',
					className
				)}
			>
				<div className="relative mr-1 shrink-0 grow-0">
					<Icon name="Folder" size={18} />
					<div
						className={clsx(
							'absolute bottom-0.5 right-0 size-1.5 rounded-full',
							online ? 'bg-green-500' : 'bg-red-500'
						)}
					/>
				</div>

				<span className="truncate">{location.name}</span>
			</SidebarLink>
		</ContextMenu>
	);
};
