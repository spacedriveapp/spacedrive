import { keepPreviousData } from '@tanstack/react-query';
import clsx from 'clsx';
import { Link, useMatch } from 'react-router-dom';

import {
	arraysEqual,
	Location as LocationType,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import { useExplorerDroppable } from '~/app/$libraryId/Explorer/useExplorerDroppable';
import { useExplorerSearchParams } from '~/app/$libraryId/Explorer/util';
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { Icon, SubtleButton } from '~/components';
import { useLocale } from '~/hooks';

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

	const { t } = useLocale();

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
				{locations?.map(location => (
					<Location
						key={location.id}
						location={location}
						online={onlineLocations.some(l => arraysEqual(location.pub_id, l))}
					/>
				))}
			</SeeMore>
			<AddLocationButton className="mt-1" />
		</Section>
	);
}

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
