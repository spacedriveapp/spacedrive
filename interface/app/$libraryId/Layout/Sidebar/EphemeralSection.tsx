import { EjectSimple } from '@phosphor-icons/react';
import { useQuery } from '@tanstack/react-query';
import clsx from 'clsx';
import { useMemo, useState } from 'react';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import { Button, toast, tw } from '@sd/ui';
import { Icon, IconName } from '~/components';
import { usePlatform } from '~/util/Platform';

import SidebarLink from './Link';
import Section from './Section';
import SeeMore from './SeeMore';

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
	const platform = usePlatform();

	const homeDir = useQuery(['userDirs', 'home'], () => {
		if (platform.userHomeDir) return platform.userHomeDir();
		else return null;
	});

	const locations = useLibraryQuery(['locations.list']);

	const volumes = useBridgeQuery(['volumes.list']);

	// this will return an array of location ids that are also volumes
	// { "/Mount/Point": 1, "/Mount/Point2": 2"}
	type LocationIdsMap = {
		[key: string]: number;
	};

	const locationIdsForVolumes = useMemo<LocationIdsMap>(() => {
		if (!locations.data || !volumes.data) return {};

		const volumePaths = volumes.data.map((volume) => volume.mount_points[0] ?? null);

		const matchedLocations = locations.data.filter((location) =>
			volumePaths.includes(location.path)
		);

		const locationIdsMap = matchedLocations.reduce((acc, location) => {
			if (location.path) {
				acc[location.path] = location.id;
			}
			return acc;
		}, {} as LocationIdsMap);

		return locationIdsMap;
	}, [locations.data, volumes.data]);

	const items = [
		{ type: 'network' },
		homeDir.data ? { type: 'home', path: homeDir.data } : null,
		...(volumes.data || []).flatMap((volume, volumeIndex) =>
			volume.mount_points.map((mountPoint, index) =>
				mountPoint !== homeDir.data
					? { type: 'volume', volume, mountPoint, volumeIndex, index }
					: null
			)
		)
	].filter(Boolean) as Array<{
		type: string;
		path?: string;
		volume?: any;
		mountPoint?: string;
		volumeIndex?: number;
		index?: number;
	}>;

	return (
		<Section name="Local">
			<SeeMore
				items={items}
				renderItem={(item, index) => {
					const locationId = locationIdsForVolumes[item.mountPoint ?? ''];

					if (item?.type === 'network') {
						return (
							<SidebarLink
								className="group relative w-full"
								to="./network"
								key={index}
							>
								<SidebarIcon name="Globe" />
								<Name>Network</Name>
							</SidebarLink>
						);
					}

					if (item?.type === 'home') {
						return (
							<SidebarLink
								to={`ephemeral/0?path=${item.path}`}
								className="group relative w-full border border-transparent"
								key={index}
							>
								<SidebarIcon name="Home" />
								<Name>Home</Name>
							</SidebarLink>
						);
					}

					if (item?.type === 'volume') {
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
					}

					return null; // This should never be reached, but is here to satisfy TypeScript
				}}
			/>
		</Section>
	);
};
