import { Laptop } from '@sd/assets/icons';
import clsx from 'clsx';
import { Link, NavLink } from 'react-router-dom';
import { arraysEqual, useBridgeQuery, useLibraryQuery, useOnlineLocations } from '@sd/client';
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { Folder } from '~/components/Folder';
import { SubtleButton } from '~/components/SubtleButton';
import SidebarLink from './Link';
import LocationsContextMenu from './LocationsContextMenu';
import Section from './Section';
import TagsContextMenu from './TagsContextMenu';

export const LibrarySection = () => {
	const node = useBridgeQuery(['nodeState']);
	const locations = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();

	return (
		<>
			<Section
				name="Nodes"
				actionArea={
					<Link to="settings/library/nodes">
						<SubtleButton />
					</Link>
				}
			>
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
								className="group relative w-full"
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
								<SidebarLink to={`tag/${tag.id}`}>
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
