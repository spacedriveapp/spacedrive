import { X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { Link, NavLink } from 'react-router-dom';
import {
	arraysEqual,
	useBridgeQuery,
	useFeatureFlag,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import { Button, Tooltip } from '@sd/ui';
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { Folder, Icon, SubtleButton } from '~/components';

import { useSavedSearches } from '../../Explorer/Search/SavedSearches';
import SidebarLink from './Link';
import LocationsContextMenu from './LocationsContextMenu';
import Section from './Section';
import SeeMore from './SeeMore';
import TagsContextMenu from './TagsContextMenu';

type SidebarGroup = {
	name: string;
	items: SidebarItem[];
};

type SidebarItem = {
	name: string;
	icon: React.ReactNode;
	to: string;
	position: number;
};

type TriggeredContextItem =
	| {
			type: 'location';
			locationId: number;
	  }
	| {
			type: 'tag';
			tagId: number;
	  };

export const LibrarySection = () => {
	const node = useBridgeQuery(['nodeState']);
	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();
	const isPairingEnabled = useFeatureFlag('p2pPairing');
	const [showDummyNodesEasterEgg, setShowDummyNodesEasterEgg] = useState(false);
	const [triggeredContextItem, setTriggeredContextItem] = useState<TriggeredContextItem | null>(
		null
	);

	// const savedSearches = useSavedSearches();

	useEffect(() => {
		const outsideClick = () => {
			document.addEventListener('click', () => {
				setTriggeredContextItem(null);
			});
		};
		outsideClick();
		return () => {
			document.removeEventListener('click', outsideClick);
		};
	}, [triggeredContextItem]);

	return (
		<>
			{/* {savedSearches.searches.length > 0 && (
				<Section
					name="Saved"
					// actionArea={
					// 	<Link to="settings/library/saved-searches">
					// 		<SubtleButton />
					// 	</Link>
					// }
				>
					<SeeMore
						items={savedSearches.searches}
						renderItem={(search) => (
							<SidebarLink
								className="group/button relative w-full"
								to={`search/${search.id}`}
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
									onClick={() => savedSearches.removeSearch(search.id)}
								>
									<X weight="bold" className="text-ink-dull/50" />
								</Button>
							</SidebarLink>
						)}
					/>
				</Section>
			)} */}
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

			<Section
				name="Locations"
				actionArea={
					<Link to="settings/library/locations">
						<SubtleButton />
					</Link>
				}
			>
				<SeeMore
					items={locationsQuery.data || []}
					renderItem={(location, index) => (
						<LocationsContextMenu key={location.id} locationId={location.id}>
							<SidebarLink
								onContextMenu={() =>
									setTriggeredContextItem({
										type: 'location',
										locationId: location.id
									})
								}
								className={clsx(
									triggeredContextItem?.type === 'location' &&
										triggeredContextItem.locationId === location.id
										? 'border-accent'
										: 'border-transparent',
									'group relative w-full border'
								)}
								to={`location/${location.id}`}
							>
								<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
									<Icon name="Folder" size={18} />
									<div
										className={clsx(
											'absolute bottom-0.5 right-0 h-1.5 w-1.5 rounded-full',
											onlineLocations.some((l) =>
												arraysEqual(location.pub_id, l)
											)
												? 'bg-green-500'
												: 'bg-red-500'
										)}
									/>
								</div>

								<span className="truncate">{location.name}</span>
							</SidebarLink>
						</LocationsContextMenu>
					)}
				/>
				<AddLocationButton className="mt-1" />
			</Section>
			{!!tags.data?.length && (
				<Section
					name="Tags"
					actionArea={
						<NavLink to="settings/library/tags">
							<SubtleButton />
						</NavLink>
					}
				>
					<SeeMore
						items={tags.data}
						renderItem={(tag, index) => (
							<TagsContextMenu tagId={tag.id} key={tag.id}>
								<SidebarLink
									onContextMenu={() =>
										setTriggeredContextItem({
											type: 'tag',
											tagId: tag.id
										})
									}
									className={clsx(
										triggeredContextItem?.type === 'tag' &&
											triggeredContextItem?.tagId === tag.id
											? 'border-accent'
											: 'border-transparent',
										'border'
									)}
									to={`tag/${tag.id}`}
								>
									<div
										className="h-[12px] w-[12px] shrink-0 rounded-full"
										style={{ backgroundColor: tag.color || '#efefef' }}
									/>
									<span className="ml-1.5 truncate text-sm">{tag.name}</span>
								</SidebarLink>
							</TagsContextMenu>
						)}
					/>
				</Section>
			)}
		</>
	);
};
