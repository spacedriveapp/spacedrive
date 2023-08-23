import { Laptop, Mobile, Server } from '@sd/assets/icons';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { Link, NavLink } from 'react-router-dom';
import {
	arraysEqual,
	useBridgeQuery,
	useDebugState,
	useFeatureFlag,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import { Button, Tooltip } from '@sd/ui';
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { Folder, SubtleButton } from '~/components';
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

const SEE_MORE_LOCATIONS_COUNT = 5;

export const LibrarySection = () => {
	const debugState = useDebugState();
	const node = useBridgeQuery(['nodeState']);
	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();
	const isPairingEnabled = useFeatureFlag('p2pPairing');
	const [triggeredContextItem, setTriggeredContextItem] = useState<TriggeredContextItem | null>(
		null
	);

	const [seeMoreLocations, setSeeMoreLocations] = useState(false);

	const locations = locationsQuery.data?.slice(
		0,
		seeMoreLocations ? undefined : SEE_MORE_LOCATIONS_COUNT
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
					isPairingEnabled && (
						<Link to="settings/library/nodes">
							<SubtleButton />
						</Link>
					)
				}
			>
				{node.data && (
					<>
						<SidebarLink
							className="group relative w-full"
							to={`node/${node.data.id}`}
							key={node.data.id}
						>
							<img src={Laptop} className="mr-1 h-5 w-5" />
							<span className="truncate">{node.data.name}</span>
						</SidebarLink>
						{debugState.enabled && (
							<>
								<SidebarLink
									className="group relative w-full"
									to={`node/23`}
									key={23}
								>
									<img src={Mobile} className="mr-1 h-5 w-5" />
									<span className="truncate">Spacephone</span>
								</SidebarLink>
								<SidebarLink
									className="group relative w-full"
									to={`node/24`}
									key={24}
								>
									<img src={Server} className="mr-1 h-5 w-5" />
									<span className="truncate">Titan</span>
								</SidebarLink>
							</>
						)}
					</>
				)}
				<Tooltip
					label="Coming soon! This alpha release doesn't include library sync, it will be ready very soon."
					tooltipClassName="bg-black"
					position="right"
				>
					<Button disabled variant="dotted" className="mt-1 w-full">
						Connect Node
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
				{locations?.map((location) => {
					const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));

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
				{locationsQuery.data?.[SEE_MORE_LOCATIONS_COUNT] && (
					<div
						onClick={() => setSeeMoreLocations(!seeMoreLocations)}
						className="mb-1 ml-2 mt-0.5 cursor-pointer text-center text-tiny font-semibold text-ink-faint/50 transition hover:text-accent"
					>
						See {seeMoreLocations ? 'less' : 'more'}
					</div>
				)}
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
