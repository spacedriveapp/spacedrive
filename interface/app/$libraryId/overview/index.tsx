import { Key } from 'react';
import { Link } from 'react-router-dom';
import { HardwareModel, useBridgeQuery, useLibraryQuery } from '@sd/client';
import { useLocale, useOperatingSystem } from '~/hooks';
import { useRouteTitle } from '~/hooks/useRouteTitle';
import { hardwareModelToIcon } from '~/util/hardware';

import { SearchContextProvider, useSearchFromSearchParams } from '../search';
import SearchBar from '../search/SearchBar';
import { AddLocationButton } from '../settings/library/locations/AddLocationButton';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import FileKindStatistics from './FileKindStats';
import OverviewSection from './Layout/Section';
import LibraryStatistics from './LibraryStats';
import NewCard from './NewCard';
import StatisticItem from './StatCard';

export interface FileKind {
	kind: number;
	name: string;
	count: bigint;
	total_bytes: bigint;
}

export const Component = () => {
	useRouteTitle('Overview');
	const os = useOperatingSystem();

	const { t } = useLocale();

	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const locations = locationsQuery.data ?? [];

	// not sure if we'll need the node state in the future, as it should be returned with the cloud.devices.list query
	// const { data: node } = useBridgeQuery(['nodeState']);
	const cloudDevicesList = useBridgeQuery(['cloud.devices.list'], {
		suspense: true,
		retry: false
	});

	const search = useSearchFromSearchParams({ defaultTarget: 'paths' });

	return (
		<SearchContextProvider search={search}>
			<div>
				<TopBarPortal
					left={
						<div className="flex items-center gap-2">
							<span className="truncate text-sm font-medium">
								{t('library_overview')}
							</span>
						</div>
					}
					center={<SearchBar redirectToSearch />}
					right={os === 'windows' && <TopBarOptions />}
				/>
				<div className="mt-4 flex flex-col gap-3 pt-3">
					<OverviewSection>
						<LibraryStatistics />
						<FileKindStatistics />
					</OverviewSection>

					<OverviewSection count={cloudDevicesList.data?.length} title={t('devices')}>
						{cloudDevicesList.data?.map(
							(
								device: {
									pub_id: Key | null | undefined;
									name: string;
									os: string;
									storage_size: bigint;
									used_storage: bigint;
									created_at: string;
									device_model: string;
								},
								index: number
							) => (
								<StatisticItem
									key={device.pub_id}
									name={device.name}
									icon={hardwareModelToIcon(device.device_model as HardwareModel)}
									// conversion to string is intentional to provide proper type to StatCardProps
									totalSpace={device.storage_size.toString()}
									freeSpace={device.used_storage.toString()}
									color="#0362FF"
									connectionType={null}
								></StatisticItem>
							)
						)}
					</OverviewSection>

					<OverviewSection count={locations.length} title={t('locations')}>
						{locations?.map((item) => (
							<Link key={item.id} to={`../location/${item.id}`}>
								<StatisticItem
									name={item.name || t('unnamed_location')}
									icon="Folder"
									totalSpace={item.size_in_bytes || [0]}
									color="#0362FF"
									connectionType={null}
								/>
							</Link>
						))}
						{!locations?.length && (
							<NewCard
								icons={['HDD', 'Folder', 'Globe', 'SD']}
								text={t('add_location_overview_description')}
								button={() => <AddLocationButton variant="outline" />}
							/>
						)}
					</OverviewSection>

					<OverviewSection count={0} title={t('cloud_drives')}>
						<NewCard
							icons={[
								'DriveAmazonS3',
								'DriveDropbox',
								'DriveGoogleDrive',
								'DriveOneDrive'
								// 'DriveBox'
							]}
							text={t('connect_cloud_description')}
							// buttonText={t('connect_cloud)}
						/>
					</OverviewSection>
				</div>
			</div>
		</SearchContextProvider>
	);
};
