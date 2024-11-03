import { ArrowRight, EjectSimple } from '@phosphor-icons/react';
import { useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import { MouseEvent, PropsWithChildren, useMemo } from 'react';
import {
	useBridgeQuery,
	useLibraryMutation,
	useLibraryQuery,
	useLibrarySubscription,
	Volume
} from '@sd/client';
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

// Improved eject button that actually unmounts the volume
const EjectButton = ({
	fingerprint,
	className
}: {
	fingerprint: Uint8Array;
	className?: string;
}) => {
	const unmountMutation = useLibraryMutation('volumes.unmount');

	return (
		<Button
			className={clsx('absolute right-[2px] !p-[5px]', className)}
			variant="subtle"
			onClick={async (e: MouseEvent) => {
				e.preventDefault(); // Prevent navigation
				try {
					await unmountMutation.mutateAsync(Array.from(fingerprint));
					toast.success('Volume ejected successfully');
				} catch (error) {
					toast.error('Failed to eject volume');
				}
			}}
		>
			<EjectSimple weight="fill" size={18} className="size-3 opacity-70" />
		</Button>
	);
};

const SidebarIcon = ({ name }: { name: IconName }) => {
	return <Icon name={name} size={20} className="mr-1" />;
};

export default function LocalSection() {
	const platform = usePlatform();
	const queryClient = useQueryClient();

	const locationsQuery = useLibraryQuery(['locations.list']);
	const locations = locationsQuery.data;

	const homeDir = useHomeDir();
	const volumesQuery = useLibraryQuery(['volumes.list']);
	const volumes = volumesQuery.data;

	const volumeEvents = useLibrarySubscription(['volumes.events'], {
		onData: (data) => {
			console.log('Volume event received:', data);
			volumesQuery.refetch();
		}
	});

	// Replace subscription with manual invalidation
	const handleVolumeChange = () => {};

	const { t } = useLocale();

	// Improved volume tracking
	const trackVolumeMutation = useLibraryMutation('volumes.track');

	// Mapping of volume paths to location IDs
	const locationIdsForVolumes = useMemo(() => {
		if (!locations || !volumes) return {};

		return locations.reduce(
			(acc, location) => {
				const matchingVolume = volumes.find((v) =>
					v.mount_points.some((mp) => mp === location.path)
				);

				if (matchingVolume && matchingVolume.pub_id && location.path) {
					acc[location.path] = {
						locationId: location.id,
						volumeId: new Uint8Array(matchingVolume.pub_id)
					};
				}

				return acc;
			},
			{} as Record<string, { locationId: number; volumeId: Uint8Array }>
		);
	}, [locations, volumes]);

	// Filter out non-unique volumes and handle mount points
	const uniqueVolumes = useMemo(() => {
		if (!volumes) return [];

		return volumes.filter((volume) => {
			if (volume.mount_type === 'System' && volume.name === 'System Reserved') return false;
			if (volume.mount_points.some((mp) => mp === homeDir.data)) return false;
			return true;
		});
	}, [volumes, homeDir.data]);

	return (
		<Section name={t('local')}>
			<SeeMore>
				<SidebarLink className="group relative w-full" to="network">
					<SidebarIcon name="Globe" />
					<Name>{t('network')}</Name>
				</SidebarLink>

				{homeDir.data && (
					<EphemeralLocation
						navigateTo={`ephemeral/home?path=${homeDir.data}`}
						path={homeDir.data}
					>
						<SidebarIcon name="Home" />
						<Name>{t('home')}</Name>
					</EphemeralLocation>
				)}

				{uniqueVolumes.map((volume) => {
					const mountPoint = volume.mount_points[0];
					if (!mountPoint) return null;
					const key = `${volume.pub_id}-${mountPoint}`;

					const locationInfo = locationIdsForVolumes[mountPoint];
					const isTracked = locationInfo !== undefined;

					const toPath = isTracked
						? `location/${locationInfo.locationId}`
						: `ephemeral/${key}?path=${volume.mount_point}`;

					const displayName = mountPoint === '/' ? 'Root' : volume.name || mountPoint;

					return (
						<EphemeralLocation
							key={key}
							navigateTo={toPath}
							path={mountPoint}
							onTrack={async () => {
								if (!isTracked && volume.pub_id) {
									try {
										await trackVolumeMutation.mutateAsync({
											volume_id: Array.from(volume.pub_id) // Convert Uint8Array to number[]
										});
										toast.success('Volume tracked successfully');
									} catch (error) {
										toast.error('Failed to track volume');
									}
								}
							}}
						>
							<SidebarIcon name={getVolumeIcon(volume)} />
							<Name>{displayName}</Name>
							{volume.mount_type === 'External' && volume.fingerprint && (
								<EjectButton fingerprint={new Uint8Array(volume.fingerprint)} />
							)}
						</EphemeralLocation>
					);
				})}
			</SeeMore>
		</Section>
	);
}

function getVolumeIcon(volume: Volume): IconName {
	if (volume.file_system === 'ExFAT') return 'SD';
	if (volume.name === 'Macintosh HD') return 'HDD';
	if (volume.disk_type === 'SSD') return 'HDD';
	if (volume.mount_type === 'Network') return 'Globe';
	if (volume.mount_type === 'External') return 'SD';
	return 'Drive';
}

// Updated EphemeralLocation component to handle tracking separately
const EphemeralLocation = ({
	children,
	path,
	navigateTo,
	onTrack
}: PropsWithChildren<{
	path: string;
	navigateTo: string;
	onTrack?: () => Promise<void>;
}>) => {
	const [{ path: ephemeralPath }] = useExplorerSearchParams();

	const { isDroppable, className, setDroppableRef } = useExplorerDroppable({
		id: `sidebar-ephemeral-location-${path}`,
		allow: ['Path', 'NonIndexedPath', 'Object'],
		data: { type: 'location', path },
		disabled: navigateTo.startsWith('location/') || ephemeralPath === path,
		navigateTo: navigateTo
		// onNavigate: onTrack
	});

	return (
		<SidebarLink
			ref={setDroppableRef}
			to={navigateTo}
			className={clsx(
				'border',
				isDroppable ? 'border-accent' : 'border-transparent',
				className
			)}
		>
			{children}
		</SidebarLink>
	);
};
