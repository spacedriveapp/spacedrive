import { Link } from 'react-router-dom';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
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

export const Component = () => {
	useRouteTitle('Overview');
	const os = useOperatingSystem();

	const { t } = useLocale();

	const locationsQuery = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const locations = locationsQuery.data ?? [];

	const { data: node } = useBridgeQuery(['nodeState']);

	const search = useSearchFromSearchParams({ defaultTarget: 'paths' });

	const stats = useLibraryQuery(['library.statistics']);

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
					right={
						os === 'windows' && <TopBarOptions />
						// <TopBarOptions
						// options={[
						// 	[
						// 		{
						// 			toolTipLabel: 'Spacedrop',
						// 			onClick: () => {},
						// 			icon: <Broadcast className={TOP_BAR_ICON_STYLE} />,
						// 			individual: true,
						// 			showAtResolution: 'sm:flex'
						// 		},
						// 		{
						// 			toolTipLabel: 'Key Manager',
						// 			onClick: () => {},
						// 			icon: <Key className={TOP_BAR_ICON_STYLE} />,
						// 			individual: true,
						// 			showAtResolution: 'sm:flex'
						// 		},
						// 		{
						// 			toolTipLabel: 'Overview Display Settings',
						// 			onClick: () => {},
						// 			icon: <SlidersHorizontal className={TOP_BAR_ICON_STYLE} />,
						// 			individual: true,
						// 			showAtResolution: 'sm:flex'
						// 		}
						// 	]
						// ]}
						// 	/>
					}
				/>
				<div className="mt-4 flex flex-col gap-3 pt-3">
					<OverviewSection>
						<LibraryStatistics />
					</OverviewSection>
					<OverviewSection>
						<FileKindStatistics />
					</OverviewSection>
					<OverviewSection count={1} title={t('devices')}>
						{node && (
							<StatisticItem
								name={node.name}
								icon={hardwareModelToIcon(node.device_model as any)}
								totalSpace={
									stats.data?.statistics?.total_local_bytes_capacity || '0'
								}
								freeSpace={stats.data?.statistics?.total_local_bytes_free || '0'}
								color="#0362FF"
								connectionType={null}
							/>
						)}
						{/* <StatisticItem
							name="Jamie's MacBook"
							icon="Laptop"
							total_space="4098046511104"
							free_space="969004651119"
							color="#0362FF"
							connection_type="p2p"
						/>
						<StatisticItem
							name="Jamie's iPhone"
							icon="Mobile"
							total_space="500046511104"
							free_space="39006511104"
							color="#0362FF"
							connection_type="p2p"
						/>
						<StatisticItem
							name="Titan NAS"
							icon="Server"
							total_space="60000046511104"
							free_space="43000046511104"
							color="#0362FF"
							connection_type="p2p"
						/>
						<StatisticItem
							name="Jamie's iPad"
							icon="Tablet"
							total_space="1074077906944"
							free_space="121006553275"
							color="#0362FF"
							connection_type="lan"
						/>
						<StatisticItem
							name="Jamie's Air"
							icon="Laptop"
							total_space="4098046511104"
							free_space="969004651119"
							color="#0362FF"
							connection_type="p2p"
						/> */}
						<NewCard
							icons={['Laptop', 'Server', 'SilverBox', 'Tablet']}
							text={t('connect_device_description')}
							className="h-auto"
							// buttonText={t('connect_device')}
						/>
						{/**/}
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
						{/* <StatisticItem
							name="James Pine"
							icon="DriveDropbox"
							total_space="104877906944"
							free_space="074877906944"
							color="#0362FF"
							connection_type="cloud"
						/>
						<StatisticItem
							name="Spacedrive S3"
							icon="DriveAmazonS3"
							total_space="1074877906944"
							free_space="704877906944"
							color="#0362FF"
							connection_type="cloud"
						/> */}

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

					{/* <OverviewSection title="Locations">
						<div className="flex flex-row gap-2">
							{locations.map((location) => (
								<div
									key={location.id}
									className="flex w-[100px] flex-col items-center gap-2"
								>
									<Icon size={80} name="Folder" />
									<span className="text-xs font-medium truncate">
										{location.name}
									</span>
								</div>
							))}
						</div>
					</OverviewSection> */}
				</div>
			</div>
		</SearchContextProvider>
	);
};
