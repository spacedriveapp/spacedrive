import { X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useMatch, useNavigate, useResolvedPath } from 'react-router';
import { Link, NavLink } from 'react-router-dom';
import {
	arraysEqual,
	useBridgeQuery,
	useFeatureFlag,
	useLibraryMutation,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import { Button, Tooltip } from '@sd/ui';
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { Folder, Icon, SubtleButton } from '~/components';

import SidebarLink from './Link';
import LocationsContextMenu from './LocationsContextMenu';
import Section from './Section';
import { SeeMore } from './SeeMore';
import TagsContextMenu from './TagsContextMenu';

export const LibrarySection = () => (
	<>
		<SavedSearches />
		<Devices />
		<Locations />
		<Tags />
	</>
);

function SavedSearches() {
	const savedSearches = useLibraryQuery(['search.saved.list']);

	const path = useResolvedPath('saved-search/:id');
	const match = useMatch(path.pathname);
	const currentSearchId = match?.params?.id;

	const currentIndex = currentSearchId
		? savedSearches.data?.findIndex((s) => s.id === Number(currentSearchId))
		: undefined;

	const navigate = useNavigate();

	const deleteSavedSearch = useLibraryMutation(['search.saved.delete'], {
		onSuccess() {
			if (currentIndex !== undefined && savedSearches.data) {
				const nextIndex = Math.min(currentIndex + 1, savedSearches.data.length - 2);

				const search = savedSearches.data[nextIndex];

				if (search) navigate(`saved-search/${search.id}`);
				else navigate(`./`);
			}
		}
	});

	if (!savedSearches.data || savedSearches.data.length < 1) return null;

	return (
		<Section
			name="Saved Searches"
			// actionArea={
			// 	<Link to="settings/library/saved-searches">
			// 		<SubtleButton />
			// 	</Link>
			// }
		>
			<SeeMore>
				{savedSearches.data.map((search, i) => (
					<SidebarLink
						className="group/button relative w-full"
						to={`saved-search/${search.id}`}
						key={search.id}
					>
						<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
							<Folder size={18} />
						</div>

						<span className="truncate">{search.name}</span>

						<Button
							className="absolute right-[2px] top-[2px] hidden rounded-full shadow group-hover/button:block"
							size="icon"
							variant="subtle"
							onClick={(e) => {
								e.preventDefault();
								e.stopPropagation();

								deleteSavedSearch.mutate(search.id);
							}}
						>
							<X size={10} weight="bold" className="text-ink-dull/50" />
						</Button>
					</SidebarLink>
				))}
			</SeeMore>
		</Section>
	);
}

function Devices() {
	const node = useBridgeQuery(['nodeState']);
	const isPairingEnabled = useFeatureFlag('p2pPairing');

	return (
		<Section
			name="Devices"
			actionArea={
				isPairingEnabled && (
					<Link to="settings/library/nodes">
						<SubtleButton />
					</Link>
				)
			}
		>
			{node.data && (
				<SidebarLink
					className="group relative w-full"
					to={`node/${node.data.id}`}
					key={node.data.id}
				>
					<Icon name="Laptop" size={20} className="mr-1" />
					<span className="truncate">{node.data.name}</span>
				</SidebarLink>
			)}
			<Tooltip
				label="Coming soon! This alpha release doesn't include library sync, it will be ready very soon."
				position="right"
			>
				<Button disabled variant="dotted" className="mt-1 w-full">
					Add Device
				</Button>
			</Tooltip>
		</Section>
	);
}

function Locations() {
	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();

	return (
		<Section
			name="Locations"
			actionArea={
				<Link to="settings/library/locations">
					<SubtleButton />
				</Link>
			}
		>
			<SeeMore>
				{locationsQuery.data?.map((location) => (
					<LocationsContextMenu key={location.id} locationId={location.id}>
						<SidebarLink
							className="borderradix-state-closed:border-transparent group relative w-full radix-state-open:border-accent"
							to={`location/${location.id}`}
						>
							<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
								<Icon name="Folder" size={18} />
								<div
									className={clsx(
										'absolute bottom-0.5 right-0 h-1.5 w-1.5 rounded-full',
										onlineLocations.some((l) => arraysEqual(location.pub_id, l))
											? 'bg-green-500'
											: 'bg-red-500'
									)}
								/>
							</div>

							<span className="truncate">{location.name}</span>
						</SidebarLink>
					</LocationsContextMenu>
				))}
			</SeeMore>
			<AddLocationButton className="mt-1" />
		</Section>
	);
}

function Tags() {
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });

	if (!tags.data?.length) return;

	return (
		<Section
			name="Tags"
			actionArea={
				<NavLink to="settings/library/tags">
					<SubtleButton />
				</NavLink>
			}
		>
			<SeeMore>
				{tags.data?.map((tag) => (
					<TagsContextMenu tagId={tag.id} key={tag.id}>
						<SidebarLink
							className="border radix-state-closed:border-transparent radix-state-open:border-accent"
							to={`tag/${tag.id}`}
						>
							<div
								className="h-[12px] w-[12px] shrink-0 rounded-full"
								style={{ backgroundColor: tag.color || '#efefef' }}
							/>
							<span className="ml-1.5 truncate text-sm">{tag.name}</span>
						</SidebarLink>
					</TagsContextMenu>
				))}
			</SeeMore>
		</Section>
	);
}
