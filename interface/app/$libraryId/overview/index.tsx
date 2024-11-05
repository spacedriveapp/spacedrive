import { keepPreviousData } from '@tanstack/react-query';
import clsx from 'clsx';
import { Key, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { HardwareModel, useBridgeQuery, useLibraryQuery } from '@sd/client';
import { Card } from '@sd/ui';
import { useAccessToken, useLocale, useOperatingSystem } from '~/hooks';
import { useRouteTitle } from '~/hooks/useRouteTitle';
import { hardwareModelToIcon } from '~/util/hardware';

import { SearchContextProvider, useSearchFromSearchParams } from '../search';
import SearchBar from '../search/SearchBar';
import { AddLocationButton } from '../settings/library/locations/AddLocationButton';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import FavoriteItems from './cards/FavoriteItems';
import FileKindStatistics from './cards/FileKindStats';
import LibraryStatistics from './cards/LibraryStats';
import RecentItems from './cards/RecentItems';
import RecentLocationsList from './cards/RecentLocationsList';
import OverviewSection from './Layout/Section';
import NewCard from './NewCard';
import StatisticItem from './StatCard';

export interface FileKind {
	kind: number;
	name: string;
	count: bigint;
	total_bytes: bigint;
}

export function OverviewCard({
	children,
	className
}: {
	children: React.ReactNode;
	className?: string;
}) {
	return (
		<Card
			className={clsx(
				'hover:bg-app-dark-box flex h-[220px] flex-col overflow-hidden bg-app-box/70 p-4 transition-colors',
				className
			)}
		>
			{children}
		</Card>
	);
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
				<div className="grid grid-cols-1 gap-4 p-4 sm:grid-cols-2 xl:grid-cols-4">
					<div className="col-span-1 sm:col-span-2 xl:col-span-4">
						<LibraryStatistics />
					</div>
					<div className="col-span-1 sm:col-span-1">
						<FavoriteItems />
					</div>
					<div className="col-span-1 sm:col-span-1 xl:col-span-2">
						<RecentLocationsList />
					</div>
					<FileKindStatistics />
					<div className="col-span-1 sm:col-span-2 xl:col-span-4">
						<RecentItems />
					</div>
				</div>
			</div>
		</SearchContextProvider>
	);
};
