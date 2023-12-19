import { ArrowClockwise, X } from '@phosphor-icons/react';
import {
	DriveAmazonS3,
	DriveDropbox,
	DriveGoogleDrive,
	Laptop,
	Mobile,
	Server,
	SilverBox,
	Tablet
} from '@sd/assets/icons';
import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { useEffect, useMemo, useState } from 'react';
import { byteSize, useCache, useDiscoveredPeers, useLibraryQuery, useNodes } from '@sd/client';
import { Button, Card, CircularProgress, tw } from '@sd/ui';
import { Icon, IconName } from '~/components';
import { useIsDark } from '~/hooks';
import { useRouteTitle } from '~/hooks/useRouteTitle';

import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { SearchContextProvider, useSearch } from '../Search';
import SearchBar from '../Search/SearchBar';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions, { TOP_BAR_ICON_STYLE } from '../TopBar/TopBarOptions';
import FileKindStatistics from './FileKindStatistics';
import NewCard from './NewCard';
import OverviewSection from './OverviewSection';
import LibraryStatistics from './Statistics';

type StatisticItemProps = {
	name: string;
	icon: string;
	total_space: string;
	free_space: string;
	color: string;
	connection_type: 'lan' | 'p2p' | 'cloud';
};

const Pill = tw.div`px-1.5 py-[1px] rounded text-tiny font-medium text-ink-dull bg-app-box border border-app-line`;

const StatisticItem = ({ icon, name, connection_type, ...stats }: StatisticItemProps) => {
	const [mounted, setMounted] = useState(false);

	const isDark = useIsDark();

	const { total_space, free_space, remaining_space } = useMemo(() => {
		return {
			total_space: byteSize(stats.total_space),
			free_space: byteSize(stats.free_space),
			remaining_space: byteSize(Number(stats.total_space) - Number(stats.free_space))
		};
	}, [stats]);

	useEffect(() => {
		setMounted(true);
	}, []);

	const progress = useMemo(() => {
		if (!mounted) return 0;
		return Math.floor(
			((Number(total_space.original) - Number(free_space.original)) /
				Number(total_space.original)) *
				100
		);
	}, [total_space, free_space, mounted]);

	return (
		<Card className="flex w-[280px] shrink-0 flex-col bg-app-box/50 !p-0 ">
			<div className="flex flex-row items-center justify-center gap-5 p-4 px-8 ">
				<CircularProgress
					radius={40}
					progress={progress}
					strokeWidth={6}
					trackStrokeWidth={6}
					strokeColor={progress > 90 ? '#E14444' : '#2599FF'}
					fillColor="transparent"
					trackStrokeColor={isDark ? '#252631' : '#efefef'}
					strokeLinecap="square"
					className="flex items-center justify-center"
					transition="stroke-dashoffset 1s ease 0s, stroke 1s ease"
				>
					<div className="absolute text-lg font-semibold">
						{remaining_space.value}
						<span className="ml-0.5 text-tiny opacity-60">{remaining_space.unit}</span>
					</div>
				</CircularProgress>
				<div className="flex flex-col">
					<img src={icon} className="h-16 w-16" />
					<span className="truncate font-medium">{name}</span>
					<span className="mt-1 truncate text-tiny text-ink-faint">
						{free_space.value}
						{free_space.unit} free of {total_space.value}
						{total_space.unit}
					</span>
				</div>
			</div>
			<div className="flex h-10 flex-row items-center gap-1.5  border-t border-app-line px-2">
				<Pill className="uppercase">{connection_type}</Pill>
				<div className="grow" />
				<Button size="icon" variant="outline">
					<Ellipsis className="h-3 w-3 opacity-50" />
				</Button>
			</div>
		</Card>
	);
};

export const Component = () => {
	useRouteTitle('Overview');
	const stats = useLibraryQuery(['library.statistics'], {
		refetchOnWindowFocus: false,
		initialData: { total_bytes_capacity: '0', library_db_size: '0' }
	});
	const locationsQuery = useLibraryQuery(['locations.list'], {
		refetchOnWindowFocus: false
	});
	useNodes(locationsQuery.data?.nodes);
	const locations = useCache(locationsQuery.data?.items);

	const discoveredPeers = useDiscoveredPeers();

	const libraryTotals = useMemo(() => {
		if (locations && discoveredPeers) {
			const capacity = byteSize(stats.data?.total_bytes_capacity);
			const free_space = byteSize(stats.data?.total_bytes_free);
			const db_size = byteSize(stats.data?.library_db_size);
			const preview_media = byteSize(stats.data?.preview_media_bytes);

			return { capacity, free_space, db_size, preview_media };
		}
	}, [locations, discoveredPeers, stats]);

	const search = useSearch({
		open: true
	});

	return (
		<SearchContextProvider search={search}>
			<div>
				<TopBarPortal
					left={
						<div className="flex items-center gap-2">
							<span className="truncate text-sm font-medium">Library Overview</span>
							{/* <Button className="!p-[5px]" variant="subtle">
							<Ellipsis className="h- w-3 opacity-50" />
						</Button> */}
						</div>
					}
					center={<SearchBar />}
					right={
						<TopBarOptions
							options={[
								[
									{
										toolTipLabel: 'Reload',
										onClick: () => {},
										icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
										individual: true,
										showAtResolution: 'xl:flex'
									},
									{
										toolTipLabel: 'Reload',
										onClick: () => {},
										icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
										individual: true,
										showAtResolution: 'xl:flex'
									},
									{
										toolTipLabel: 'Reload',
										onClick: () => {},
										icon: <ArrowClockwise className={TOP_BAR_ICON_STYLE} />,
										individual: true,
										showAtResolution: 'xl:flex'
									}
								]
							]}
						/>
					}
				/>
				<div className="mt-4 flex flex-col gap-3 pt-3">
					<OverviewSection>
						<LibraryStatistics />
					</OverviewSection>
					<OverviewSection>
						<FileKindStatistics />
					</OverviewSection>
					<OverviewSection count={8} title="Devices">
						<StatisticItem
							name="Jam Macbook Pro"
							icon={Laptop}
							total_space="1074077906944"
							free_space="121006553275"
							color="#0362FF"
							connection_type="lan"
						/>
						<StatisticItem
							name="Spacestudio"
							icon={SilverBox}
							total_space="4098046511104"
							free_space="969004651119"
							color="#0362FF"
							connection_type="p2p"
						/>
						<StatisticItem
							name="Jamie's iPhone"
							icon={Mobile}
							total_space="500046511104"
							free_space="39006511104"
							color="#0362FF"
							connection_type="p2p"
						/>
						<StatisticItem
							name="Titan NAS"
							icon={Server}
							total_space="60000046511104"
							free_space="43000046511104"
							color="#0362FF"
							connection_type="p2p"
						/>
						<StatisticItem
							name="Jamie's iPad"
							icon={Tablet}
							total_space="1074077906944"
							free_space="121006553275"
							color="#0362FF"
							connection_type="lan"
						/>
						<StatisticItem
							name="Jamie's Air"
							icon={Laptop}
							total_space="4098046511104"
							free_space="969004651119"
							color="#0362FF"
							connection_type="p2p"
						/>
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
						icon={DriveDropbox}
						total_space="104877906944"
						free_space="074877906944"
						color="#0362FF"
						connection_type="cloud"
					/>
					<StatisticItem
						name="Spacedrive S3"
						icon={DriveAmazonS3}
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
