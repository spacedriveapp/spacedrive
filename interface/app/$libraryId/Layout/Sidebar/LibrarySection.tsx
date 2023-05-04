import clsx from 'clsx';
import { Link, NavLink } from 'react-router-dom';
import { arraysEqual, useLibraryQuery, useOnlineLocations } from '@sd/client';
import { Folder } from '@sd/ui';
import { AddLocationButton } from '~/app/$libraryId/settings/library/locations/AddLocationButton';
import { SubtleButton } from '~/components/SubtleButton';
import SidebarLink from './Link';
import Section from './Section';

export const LibrarySection = () => {
	const locations = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const onlineLocations = useOnlineLocations();

	return (
		<>
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
						<SidebarLink
							className="group relative w-full"
							to={`location/${location.id}`}
							key={location.id}
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
						{tags.data?.slice(0, 6).map((tag, index) => (
							<SidebarLink key={index} to={`tag/${tag.id}`} className="">
								<div
									className="h-[12px] w-[12px] shrink-0 rounded-full"
									style={{ backgroundColor: tag.color || '#efefef' }}
								/>
								<span className="ml-1.5 truncate text-sm">{tag.name}</span>
							</SidebarLink>
						))}
					</div>
				</Section>
			)}
			{/* <Section name="Debug">
				<SidebarLink to="sync">
					<Icon component={ArrowsClockwise} />
					Sync
				</SidebarLink>
			</Section> */}
		</>
	);
};
