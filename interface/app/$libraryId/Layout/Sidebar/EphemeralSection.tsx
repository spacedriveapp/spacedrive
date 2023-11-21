import { EjectSimple } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useMemo } from 'react';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import { Button, toast, tw } from '@sd/ui';
import { Icon, IconName } from '~/components';
import { useHomeDir } from '~/hooks/useHomeDir';

import SidebarLink from './Link';
import Section from './Section';
import { SeeMore } from './SeeMore';

const Name = tw.span`truncate`;

// TODO: This eject button does nothing!
const EjectButton = ({ className }: { className?: string }) => (
	<Button
		className={clsx('absolute right-[2px] !p-[5px]', className)}
		variant="subtle"
		onClick={() => toast.info('Eject button coming soon')}
	>
		<EjectSimple weight="fill" size={18} className="h-3 w-3 opacity-70" />
	</Button>
);

const SidebarIcon = ({ name }: { name: IconName }) => {
	return <Icon name={name} size={20} className="mr-1" />;
};

export const EphemeralSection = () => {
	const locations = useLibraryQuery(['locations.list']);

	const homeDir = useHomeDir();
	const volumes = useBridgeQuery(['volumes.list']);

	// this will return an array of location ids that are also volumes
	// { "/Mount/Point": 1, "/Mount/Point2": 2"}
	const locationIdsForVolumes = useMemo(() => {
		if (!locations.data || !volumes.data) return {};

		const volumePaths = volumes.data.map((volume) => volume.mount_points[0] ?? null);

		const matchedLocations = locations.data.filter((location) =>
			volumePaths.includes(location.path)
		);

		const locationIdsMap = matchedLocations.reduce(
			(acc, location) => {
				if (location.path) {
					acc[location.path] = location.id;
				}
				return acc;
			},
			{} as {
				[key: string]: number;
			}
		);

		return locationIdsMap;
	}, [locations.data, volumes.data]);

	const mountPoints = (volumes.data || []).flatMap((volume, volumeIndex) =>
		volume.mount_points.map((mountPoint, index) =>
			mountPoint !== homeDir.data
				? { type: 'volume', volume, mountPoint, volumeIndex, index }
				: null
		)
	);

	return (
		<Section name="Local">
			<SeeMore>
				<SidebarLink className="group relative w-full" to="network">
					<SidebarIcon name="Globe" />
					<Name>Network</Name>
				</SidebarLink>
				{homeDir.data && (
					<SidebarLink
						to={`ephemeral/0?path=${homeDir.data}`}
						className="group relative w-full border border-transparent"
					>
						<SidebarIcon name="Home" />
						<Name>Home</Name>
					</SidebarLink>
				)}
				{mountPoints.map((item) => {
					if (!item) return;

					const locationId = locationIdsForVolumes[item.mountPoint ?? ''];

					const key = `${item.volumeIndex}-${item.index}`;
					const name =
						item.mountPoint === '/'
							? 'Root'
							: item.index === 0
							? item.volume.name
							: item.mountPoint;
					const toPath =
						locationId !== undefined
							? `location/${locationId}`
							: `ephemeral/${key}?path=${item.mountPoint}`;
					return (
						<SidebarLink
							to={toPath}
							key={key}
							className="group relative w-full border border-transparent"
						>
							<SidebarIcon
								name={
									item.volume.file_system === 'exfat'
										? 'SD'
										: item.volume.name === 'Macintosh HD'
										? 'HDD'
										: 'Drive'
								}
							/>
							<Name>{name}</Name>
							{item.volume.disk_type === 'Removable' && <EjectButton />}
						</SidebarLink>
					);
				})}
			</SeeMore>
		</Section>
	);
};
