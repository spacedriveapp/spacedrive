import { ArrowClockwise, Laptop } from '@phosphor-icons/react';
import { DriveAmazonS3, DriveDropbox, Mobile, Server, Tablet } from '@sd/assets/icons';
import { useBridgeQuery, useLibraryQuery, useNodes } from '@sd/client';
import { useRouteTitle } from '~/hooks/useRouteTitle';
import { hardwareModelToIcon } from '~/util/hardware';

import { SearchContextProvider, useSearch } from '../Search';
import SearchBar from '../Search/SearchBar';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions, { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import FileKindStatistics from './FileKindStatistics';
import NewCard from './NewCard';
import OverviewSection from './OverviewSection';
import StatisticItem from './StatisticItem';
import LibraryStatistics from './Statistics';

export const Component = () => {
	useRouteTitle('Overview');

	const locationsQuery = useLibraryQuery(['locations.list'], {
		refetchOnWindowFocus: false
	});
	useNodes(locationsQuery.data?.nodes);

	const { data: node } = useBridgeQuery(['nodeState']);

	const search = useSearch({
		open: true
	});

	const stats = useLibraryQuery(['library.statistics']);

	return (
		<SearchContextProvider search={search}>
			<div>
				<TopBarPortal
					left={
						<div className="flex items-center gap-2">
							<span className="truncate text-sm font-medium">Library Overview</span>
						</div>
					}
					center={<SearchBar />}
					// right={
					// 	<TopBarOptions
					// 		options={[
					// 			[
					// 				{
					// 					toolTipLabel: 'Reload',
					// 					onClick: () => {},
					// 					icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
					// 					individual: true,
					// 					showAtResolution: 'xl:flex'
					// 				},
					// 				{
					// 					toolTipLabel: 'Reload',
					// 					onClick: () => {},
					// 					icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
					// 					individual: true,
					// 					showAtResolution: 'xl:flex'
					// 				},
					// 				{
					// 					toolTipLabel: 'Reload',
					// 					onClick: () => {},
					// 					icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
					// 					individual: true,
					// 					showAtResolution: 'xl:flex'
					// 				}
					// 			]
					// 		]}
					// 	/>
					// }
				/>
				<div className="mt-4 flex flex-col gap-3 pt-3">
					<OverviewSection>
						<LibraryStatistics />
					</OverviewSection>
					<OverviewSection>
						<FileKindStatistics />
					</OverviewSection>
					<OverviewSection count={8} title="Devices">
						{node && (
							<StatisticItem
								name={node.name}
								// this is a hack, we should map the device model to the icon in a util and actually have more than two mac models lol
								icon={hardwareModelToIcon(node.device_model as any)}
								total_space={stats.data?.statistics?.total_bytes_capacity || '0'}
								free_space={stats.data?.statistics?.total_bytes_free || '0'}
								color="#0362FF"
								connection_type="lan"
							/>
						)}
						{/* <StatisticItem
							name="Jamie's Macbook"
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
							icons={['Laptop', 'Server', 'SilverBox', 'Tablet', 'Mobile']}
							text="Spacedrive works best on all your devices."
							buttonText="Connect a device"
						/>
						{/**/}
					</OverviewSection>

					<OverviewSection count={3} title="Cloud Drives">
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
								'DriveOneDrive',
								'DriveBox'
							]}
							text="Connect your cloud accounts to Spacedrive."
							buttonText="Connect a cloud"
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
									<span className="truncate text-xs  font-medium">
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
