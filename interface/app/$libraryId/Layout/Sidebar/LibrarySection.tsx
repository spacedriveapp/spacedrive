import { Laptop } from '@sd/assets/icons';
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
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { Folder } from '~/components/Folder';
import { SubtleButton } from '~/components/SubtleButton';
import SidebarLink from './Link';
import LocationsContextMenu from './LocationsContextMenu';
import Section from './Section';
import TagsContextMenu from './TagsContextMenu';

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
	const locations = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();
	const isPairingEnabled = useFeatureFlag('p2pPairing');
	const [triggeredContextItem, setTriggeredContextItem] = useState<TriggeredContextItem | null>(
		null
	);

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
			<Section
				name="Nodes"
				actionArea={
					isPairingEnabled ? (
						<Link to="settings/library/nodes">
							<SubtleButton />
						</Link>
					) : (
						<SubtleButton />
					)
				}
			>
				{/* <SidebarLink className="relative w-full group" to={`/`}>
					<img src={Laptop} className="w-5 h-5 mr-1" />
					<span className="truncate">Jamie's MBP</span>
				</SidebarLink>
				<SidebarLink className="relative w-full group" to={`/`}>
					<img src={Mobile} className="w-5 h-5 mr-1" />
					<span className="truncate">spacephone</span>
				</SidebarLink>
				<SidebarLink className="relative w-full group" to={`/`}>
					<img src={Server} className="w-5 h-5 mr-1" />
					<span className="truncate">titan</span>
				</SidebarLink>
				{(locations.data?.length || 0) < 4 && (
					<Button variant="dotted" className="w-full mt-1">
						Connect Node
					</Button>
				)} */}
				<SidebarLink disabled className="group relative w-full" to={`/`} key={'jeff'}>
					<img src={Laptop} className="mr-1 h-5 w-5" />
					<span className="truncate">{node.data?.name}</span>
				</SidebarLink>
			</Section>
			<Section
				name="Locations"
				actionArea={
					<Link to="settings/library/locations">
						<SubtleButton />
					</Link>
				}
			>
				{locations.data?.map((location) => {
					const online = onlineLocations?.some((l) => arraysEqual(location.pub_id, l));
					return (
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
									<Folder size={18} />
									<div
										className={clsx(
											'absolute bottom-0.5 right-0 h-1.5 w-1.5 rounded-full',
											online ? 'bg-green-500' : 'bg-red-500'
										)}
									/>
								</div>

								<span className="truncate">{location.name}</span>
							</SidebarLink>
						</LocationsContextMenu>
					);
				})}
				{(locations.data?.length || 0) < 4 && <AddLocationButton className="mt-1" />}
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
					<div className="mb-2 mt-1">
						{tags.data?.slice(0, 6).map((tag) => (
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
						))}
					</div>
				</Section>
			)}
		</>
	);
};
