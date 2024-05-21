import { ArrowRight, EjectSimple } from '@phosphor-icons/react';
import clsx from 'clsx';
import { PropsWithChildren, useMemo } from 'react';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import { Button, toast, tw } from '@sd/ui';
import { Icon, IconName } from '~/components';
import { useLocale } from '~/hooks';
import { useHomeDir } from '~/hooks/useHomeDir';
import { usePlatform } from '~/util/Platform';

import { useExplorerDroppable } from '../../../../Explorer/useExplorerDroppable';
import { useExplorerSearchParams } from '../../../../Explorer/util';
import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import { SeeMore } from '../../SidebarLayout/SeeMore';

const Name = tw.span`truncate`;

// TODO: This eject button does nothing!
const EjectButton = ({ className }: { className?: string }) => (
	<Button
		className={clsx('absolute right-[2px] !p-[5px]', className)}
		variant="subtle"
		onClick={() => toast.info('Eject button coming soon')}
	>
		<EjectSimple weight="fill" size={18} className="size-3 opacity-70" />
	</Button>
);

const SidebarIcon = ({ name }: { name: IconName }) => {
	return <Icon name={name} size={20} className="mr-1" />;
};

export default function LocalSection() {
	const platform = usePlatform();
	const locationsQuery = useLibraryQuery(['locations.list']);
	const locations = locationsQuery.data;

	const homeDir = useHomeDir();
	const result = useBridgeQuery(['volumes.list']);
	const volumes = result.data;

	const { t } = useLocale();

	// this will return an array of location ids that are also volumes
	// { "/Mount/Point": 1, "/Mount/Point2": 2"}
	const locationIdsForVolumes = useMemo(() => {
		if (!locations || !volumes) return {};

		const volumePaths = volumes.map((volume) => volume.mount_points[0] ?? null);

		const matchedLocations = locations.filter((location) =>
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
	}, [locations, volumes]);

	const mountPoints = (volumes || []).flatMap((volume, volumeIndex) =>
		volume.mount_points.map((mountPoint, index) =>
			mountPoint !== homeDir.data
				? { type: 'volume', volume, mountPoint, volumeIndex, index }
				: null
		)
	);

	return (
		<Section name={t('local')}>
			<SeeMore>
				<SidebarLink className="group relative w-full" to="network">
					<SidebarIcon name="Globe" />
					<Name>{t('network')}</Name>
				</SidebarLink>

				{homeDir.data && (
					<EphemeralLocation
						navigateTo={`ephemeral/0?path=${homeDir.data}`}
						path={homeDir.data ?? ''}
					>
						<SidebarIcon name="Home" />
						<Name>{t('home')}</Name>
					</EphemeralLocation>
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
						<EphemeralLocation
							key={key}
							navigateTo={toPath}
							path={
								locationId !== undefined
									? locationId.toString()
									: item.mountPoint ?? ''
							}
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
						</EphemeralLocation>
					);
				})}
			</SeeMore>
		</Section>
	);
}

const EphemeralLocation = ({
	children,
	path,
	navigateTo
}: PropsWithChildren<{ path: string; navigateTo: string }>) => {
	const [{ path: ephemeralPath }] = useExplorerSearchParams();

	const { isDroppable, className, setDroppableRef } = useExplorerDroppable({
		id: `sidebar-ephemeral-location-${path}`,
		allow: ['Path', 'NonIndexedPath', 'Object'],
		data: { type: 'location', path },
		disabled: navigateTo.startsWith('location/') || ephemeralPath === path,
		navigateTo: navigateTo
	});

	return (
		<SidebarLink
			ref={setDroppableRef}
			to={navigateTo}
			className={clsx(
				'border',
				isDroppable ? ' border-accent' : 'border-transparent',
				className
			)}
		>
			{children}
		</SidebarLink>
	);
};
