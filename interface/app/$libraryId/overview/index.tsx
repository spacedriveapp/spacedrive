import { keepPreviousData } from '@tanstack/react-query';
import { Key, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { HardwareModel, useBridgeQuery, useLibraryQuery } from '@sd/client';
import { useAccessToken, useLocale, useOperatingSystem } from '~/hooks';
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
	const accessToken = useAccessToken();

	const locationsQuery = useLibraryQuery(['locations.list'], {
		placeholderData: keepPreviousData
	});
	const locations = locationsQuery.data ?? [];

	// not sure if we'll need the node state in the future, as it should be returned with the cloud.devices.list query
	// const { data: node } = useBridgeQuery(['nodeState']);
	const cloudDevicesList = useBridgeQuery(['cloud.devices.list']);

	useEffect(() => {
		const interval = setInterval(async () => {
			await cloudDevicesList.refetch();
		}, 10000);
		return () => clearInterval(interval);
	}, []);
	const { data: node } = useBridgeQuery(['nodeState']);
	const stats = useLibraryQuery(['library.statistics']);
	console.log('stats', stats.data);

	const search = useSearchFromSearchParams({ defaultTarget: 'paths' });

	return (
		<SearchContextProvider search={search}>
			<div>
				<TopBarPortal
					left={
						<div className="flex items-center gap-2">
							<span className="truncate text-sm font-medium">{t('library_overview')}</span>
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

					<OverviewSection
						count={(cloudDevicesList.data?.length ?? 0) + (node ? 1 : 0)}
						title={t('devices')}
					>
						{node && (
							<StatisticItem
								name={node.name}
								icon={hardwareModelToIcon(node.device_model as any)}
								totalSpace={stats.data?.statistics?.total_local_bytes_capacity || '0'}
								freeSpace={stats.data?.statistics?.total_local_bytes_free || '0'}
								color="#0362FF"
								connectionType={null}
							/>
						)}
						{cloudDevicesList.data?.map((device) => (
							<StatisticItem
								key={device.pub_id}
								name={device.name}
								icon={hardwareModelToIcon(device.hardware_model as HardwareModel)}
								totalSpace="0"
								freeSpace="0"
								color="#0362FF"
								connectionType={'cloud'}
							/>
						))}
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
