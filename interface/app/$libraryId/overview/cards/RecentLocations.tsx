import { FolderDashed } from '@phosphor-icons/react';
import { keepPreviousData } from '@tanstack/react-query';
import { useMemo } from 'react';
import { useNavigate } from 'react-router';
import { arraysEqual, Device, humanizeSize, useLibraryQuery, useOnlineLocations } from '@sd/client';
import { Button, buttonStyles, Card, Tooltip } from '@sd/ui';
import { Icon as SdIcon } from '~/components';
import { useLocale } from '~/hooks';

import { OverviewCard } from '..';
import { AddLocationButton } from '../../settings/library/locations/AddLocationButton';

const RecentLocations = () => {
	const navigate = useNavigate();
	const { t } = useLocale();
	const onlineLocations = useOnlineLocations();

	const devicesQuery = useLibraryQuery(['devices.list'], {
		// placeholderData: keepPreviousData
	});
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const devices = devicesQuery.data ?? [];

	const devicesAsHashmap = useMemo(() => {
		return devices.reduce(
			(acc, device) => {
				acc[device.id] = device;
				return acc;
			},
			{} as Record<string, Device>
		);
	}, [devices]);

	const locationsQuery = useLibraryQuery(['locations.list'], {
		// placeholderData: keepPreviousData
	});
	const locations = locationsQuery.data ?? [];

	return (
		<>
			<div className="grid grid-cols-2 gap-2">
				{locations.length > 0 ? (
					locations.slice(0, 6).map((location) => (
						<button
							key={location.id}
							onClick={() => navigate(`location/${location.id}`)}
							className="flex items-center gap-3 rounded-md p-2.5 text-left hover:bg-app-selected/50"
						>
							<div className="relative shrink-0">
								<SdIcon name="Folder" size={38} />
								<div
									className={`absolute -right-0 bottom-1 size-2 rounded-full ${
										onlineLocations.some((l) => arraysEqual(location.pub_id, l))
											? 'bg-green-500'
											: 'bg-app-selected'
									}`}
								/>
							</div>

							<div className="min-w-0 flex-1">
								<div className="truncate text-sm font-medium">{location.name}</div>
								{location.device_id && (
									<div className="truncate text-xs text-ink-faint">
										on{' '}
										{devicesAsHashmap[location.device_id]?.name ??
											'Unknown Device'}
									</div>
								)}
							</div>

							<Tooltip position="top" label={t('size')}>
								<div className="shrink-0 rounded-md border border-app-selected/30 bg-app-box px-1 py-0 font-medium">
									<span className="text-[10px] text-ink-dull">
										{humanizeSize(location.size_in_bytes).value}
										<span className="ml-0.5 text-[9px] text-ink-dull/60">
											{t(
												`size_${humanizeSize(location.size_in_bytes).unit.toLowerCase()}`
											)}
										</span>
									</span>
								</div>
							</Tooltip>
						</button>
					))
				) : (
					<div>No locations found</div>
				)}
				<AddLocationButton className="mt-1 w-full" />
			</div>
		</>
	);
};

export default RecentLocations;
