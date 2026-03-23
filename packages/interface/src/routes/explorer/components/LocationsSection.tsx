import {useDroppable} from '@dnd-kit/core';
import {Plus} from '@phosphor-icons/react';
import {Location} from '@sd/assets/icons';
import type {Location} from '@sd/ts-client';
import clsx from 'clsx';
import {useEffect, useRef} from 'react';
import {useNavigate} from 'react-router-dom';
import {useNormalizedQuery} from '../../../contexts/SpacedriveContext';
import {useEvent} from '../../../hooks/useEvent';
import {useAddLocationDialog} from './AddLocationModal';
import {Section} from './Section';
import {SidebarItem} from './SidebarItem';

export function LocationsSection() {
	const navigate = useNavigate();
	const previousLocationIdsRef = useRef<Set<string>>(new Set());

	const locationsQuery = useNormalizedQuery<null, Location>({
		query: 'locations.list',
		input: null,
		resourceType: 'location'
	});

	const locations = locationsQuery.data?.locations || [];

	// Track location IDs to detect new locations
	useEffect(() => {
		previousLocationIdsRef.current = new Set(
			locations.map((loc) => loc.id)
		);
	}, [locations]);

	// Listen for new location creation events and navigate to them
	useEvent('ResourceChanged', (event) => {
		if ('ResourceChanged' in event) {
			const {resource_type, resource} = event.ResourceChanged;

			if (
				resource_type === 'location' &&
				typeof resource === 'object' &&
				resource !== null
			) {
				const newLocation = resource as Location;

				// Check if this is a new location (not in our previous set)
				if (!previousLocationIdsRef.current.has(newLocation.id)) {
					navigate(`/location/${newLocation.id}`);
				}
			}
		}
	});

	const handleAddLocation = async () => {
		// Navigation now happens automatically via ResourceChanged event
		await useAddLocationDialog();
	};

	return (
		<Section title="Locations">
			{locationsQuery.isLoading && (
				<div className="text-sidebar-inkFaint px-2 py-1 text-xs">
					Loading...
				</div>
			)}

			{locationsQuery.error && (
				<div className="px-2 py-1 text-xs text-red-400">
					Error: {(locationsQuery.error as Error).message}
				</div>
			)}

			{locations.length === 0 &&
				!locationsQuery.isLoading &&
				!locationsQuery.error && (
					<div className="text-sidebar-inkFaint px-2 py-1 text-xs">
						No locations yet
					</div>
				)}

			{locations.map((location) => (
				<LocationDropZone key={location.id} location={location} />
			))}

			<SidebarItem
				icon={Plus}
				label="Add Location"
				onClick={handleAddLocation}
				className="text-ink-faint hover:text-ink"
			/>
		</Section>
	);
}

// Location item with drop zone support
function LocationDropZone({location}: {location: Location}) {
	const {setNodeRef, isOver} = useDroppable({
		id: `location-drop-${location.id}`,
		data: {
			action: 'move-into',
			targetType: 'location',
			targetId: location.id,
			targetPath: location.sd_path // Use the proper sd_path from the location
		}
	});

	return (
		<div ref={setNodeRef} className="relative">
			{isOver && (
				<div className="ring-accent pointer-events-none absolute inset-0 z-10 rounded-lg ring-2 ring-inset" />
			)}
			<SidebarItem
				icon={Location}
				label={location.name || 'Unnamed'}
				to={`/location/${location.id}`}
				className={clsx(isOver && 'bg-accent/10')}
			/>
		</div>
	);
}
