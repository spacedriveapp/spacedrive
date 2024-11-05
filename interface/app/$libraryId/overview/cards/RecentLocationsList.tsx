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

const RecentLocationsList = () => {
	const navigate = useNavigate();
	const { t } = useLocale();
	const onlineLocations = useOnlineLocations();

	const devicesQuery = useLibraryQuery(['devices.list']);
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
		placeholderData: keepPreviousData
	});
	const locations = locationsQuery.data ?? [];

	return (
		<OverviewCard>
			<div className="mb-2 flex items-center justify-between pt-1">
				<span className="text-sm font-medium">{t('Recent Locations')}</span>
				<span className="text-xs text-ink-faint">{locations.length} total</span>
			</div>

			<div className="grid grid-cols-2 gap-2">
				{locations.slice(0, 6).map((location) => (
					<button
						key={location.id}
						onClick={() => navigate(`location/${location.id}`)}
						className="flex items-center gap-3 rounded-md p-2.5 text-left hover:bg-app-selected/50"
					>
						<div className="relative shrink-0">
							<SdIcon name="Folder" size={32} />
							<div
								className={`absolute -bottom-0.5 -right-0.5 size-2 rounded-full ${
									onlineLocations.some((l) => arraysEqual(location.pub_id, l))
										? 'bg-green-500'
										: 'bg-red-500'
								}`}
							/>
						</div>

						<div className="min-w-0 flex-1">
							<div className="truncate text-sm font-medium">{location.name}</div>
							{location.device_id && (
								<div className="truncate text-xs text-ink-faint">
									on{' '}
									{devicesAsHashmap[location.device_id]?.name ?? 'Unknown Device'}
								</div>
							)}
						</div>

						<Tooltip position="top" label={t('size')}>
							<div
								className={buttonStyles({
									variant: 'gray',
									className: 'shrink-0 !px-2 !py-0.5'
								})}
								onClick={(e) => e.stopPropagation()}
							>
								<span className="text-xs text-ink-dull">
									{humanizeSize(location.size_in_bytes).value}
									<span className="ml-0.5 text-[10px] text-ink-dull/60">
										{t(
											`size_${humanizeSize(location.size_in_bytes).unit.toLowerCase()}`
										)}
									</span>
								</span>
							</div>
						</Tooltip>
					</button>
				))}
				<AddLocationButton className="mt-1 w-full" />
			</div>
		</OverviewCard>
	);
};

export default RecentLocationsList;
