import { ExclamationCircleIcon, PlusIcon } from '@heroicons/react/solid';
import { useBridgeQuery, useLibraryQuery } from '@sd/client';
import { AppPropsContext } from '@sd/client';
import { Statistics } from '@sd/core';
import { Button, Input } from '@sd/ui';
import byteSize from 'byte-size';
import clsx from 'clsx';
import React, { useContext, useEffect, useState } from 'react';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import create from 'zustand';

import { Device } from '../components/device/Device';
import Dialog from '../components/layout/Dialog';

interface StatItemProps {
	title: string;
	bytes: string;

	isLoading: boolean;
}

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};

type OverviewStats = Partial<Record<keyof Statistics, string>>;
type OverviewState = {
	overviewStats: OverviewStats;
	setOverviewStat: (name: keyof OverviewStats, newValue: string) => void;
	setOverviewStats: (stats: OverviewStats) => void;
};

export const useOverviewState = create<OverviewState>((set) => ({
	overviewStats: {},
	setOverviewStat: (name, newValue) =>
		set((state) => ({
			...state,
			overviewStats: {
				...state.overviewStats,
				[name]: newValue
			}
		})),
	setOverviewStats: (stats) =>
		set((state) => ({
			...state,
			overviewStats: stats
		}))
}));

function quadratic(duration: number, range: number, current: number) {
	return ((duration * 3) / Math.pow(range, 3)) * Math.pow(current, 2);
}

const StatItem: React.FC<StatItemProps> = (props) => {
	const { title, bytes = '0', isLoading } = props;

	const appProps = useContext(AppPropsContext);

	const size = byteSize(+bytes);

	const [count, setCount] = useState(0);

	useEffect(() => {
		if (count < +size.value) {
			setTimeout(() => {
				setCount((count) => count + 1);
			}, quadratic(appProps?.demoMode ? 1000 : 500, +size.value, count));
		}
	}, [count, size]);

	return (
		<div
			className={clsx(
				'flex flex-col flex-shrink-0 w-32 px-4 py-3 duration-75 transform rounded-md cursor-default hover:bg-gray-50 hover:dark:bg-gray-600',
				!+bytes && 'hidden'
			)}
		>
			<span className="text-sm text-gray-400">{title}</span>
			<span className="text-2xl font-bold">
				{isLoading && (
					<div>
						<Skeleton enableAnimation={true} baseColor={'#21212e'} highlightColor={'#13131a'} />
					</div>
				)}
				<div
					className={clsx({
						hidden: isLoading
					})}
				>
					<span className="tabular-nums">{count}</span>
					<span className="ml-1 text-[16px] text-gray-400">{size.unit}</span>
				</div>
			</span>
		</div>
	);
};

export const OverviewScreen = () => {
	const { data: libraryStatistics, isLoading: isStatisticsLoading } = useLibraryQuery([
		'library.getStatistics'
	]);
	const { data: nodeState } = useBridgeQuery(['getNode']);

	const { overviewStats, setOverviewStats } = useOverviewState();

	// get app props from context
	const appProps = useContext(AppPropsContext);

	useEffect(() => {
		if (appProps?.demoMode === true) {
			if (!Object.entries(overviewStats).length)
				setOverviewStats({
					total_bytes_capacity: '8093333345230',
					preview_media_bytes: '2304387532',
					library_db_size: '83345230',
					total_file_count: '20342345',
					total_bytes_free: '89734502034',
					total_bytes_used: '8093333345230',
					total_unique_bytes: '9347397'
				});
		} else {
			const newStatistics: OverviewStats = {
				total_bytes_capacity: '0',
				preview_media_bytes: '0',
				library_db_size: '0',
				total_file_count: '0',
				total_bytes_free: '0',
				total_bytes_used: '0',
				total_unique_bytes: '0'
			};

			Object.entries((libraryStatistics as Statistics) || {}).forEach(([key, value]) => {
				newStatistics[key as keyof Statistics] = `${value}`;
			});

			setOverviewStats(newStatistics);
		}
	}, [appProps, libraryStatistics]);

	// useEffect(() => {
	// 	setTimeout(() => {
	// 		setOverviewStat('total_bytes_capacity', '4093333345230');
	// 	}, 2000);
	// }, [overviewStats]);

	const displayableStatItems = Object.keys(StatItemNames) as unknown as keyof typeof StatItemNames;

	return (
		<div className="flex flex-col w-full h-screen overflow-x-hidden custom-scroll page-scroll">
			<div data-tauri-drag-region className="flex flex-shrink-0 w-full h-5" />
			{/* PAGE */}
			<div className="flex flex-col w-full h-screen px-4">
				{/* STAT HEADER */}
				<div className="flex w-full">
					{/* STAT CONTAINER */}
					<div className="flex -mb-1 overflow-hidden">
						{!libraryStatistics && (
							<div className="mb-2 ml-2">
								<div className="font-semibold text-gray-200">
									<ExclamationCircleIcon className="inline w-4 h-4 mr-1 -mt-1 " /> Missing library
								</div>
								<span className="text-xs text-gray-400 ">
									Ensure the library you have loaded still exists on disk
								</span>
							</div>
						)}
						{Object.entries(overviewStats).map(([key, value]) => {
							if (!displayableStatItems.includes(key)) return null;

							return (
								<StatItem
									key={key}
									title={StatItemNames[key as keyof Statistics]!}
									bytes={value}
									isLoading={appProps?.demoMode === true ? false : isStatisticsLoading}
								/>
							);
						})}
					</div>

					<div className="flex-grow" />
					<div className="space-x-2 ">
						<Dialog
							title="Add Device"
							description="Connect a new device to your library. Either enter another device's code or copy this one."
							ctaAction={() => {}}
							ctaLabel="Connect"
							trigger={
								<Button
									size="sm"
									icon={<PlusIcon className="inline w-4 h-4 -mt-0.5 mr-1" />}
									variant="gray"
								>
									Add Device
								</Button>
							}
						>
							<div className="flex flex-col mt-2 space-y-3">
								<div className="flex flex-col">
									<span className="mb-1 text-xs font-bold uppercase text-gray-450">
										This Device
									</span>
									<Input readOnly disabled value="06ffd64309b24fb09e7c2188963d0207" />
								</div>
								<div className="flex flex-col">
									<span className="mb-1 text-xs font-bold uppercase text-gray-450">
										Enter a device code
									</span>
									<Input value="" />
								</div>
							</div>
						</Dialog>
					</div>
				</div>
				<div className="flex flex-col pb-4 mt-4 space-y-4">
					<Device name={`James' MacBook Pro`} size="1TB" locations={[]} type="desktop" />
					<Device name={`James' iPhone 12`} size="47.7GB" locations={[]} type="phone" />
					<Device name={`Spacedrive Server`} size="5GB" locations={[]} type="server" />
				</div>
				<div className="px-5 py-3 text-sm text-gray-400 rounded-md bg-gray-50 dark:text-gray-400 dark:bg-gray-600">
					<b>Note: </b>This is a pre-alpha build of Spacedrive, many features are yet to be
					functional.
				</div>
				<div className="flex flex-shrink-0 w-full h-4" />
			</div>
		</div>
	);
};
