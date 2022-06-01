import { PlusIcon } from '@heroicons/react/solid';
import { useBridgeQuery } from '@sd/client';
import { Statistics } from '@sd/core';
import { Button, Input } from '@sd/ui';
import byteSize from 'byte-size';
import type { ByteSizeResult } from 'byte-size';
import clsx from 'clsx';
import React, { useContext, useEffect } from 'react';
import { useCountUp } from 'react-countup';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import create from 'zustand';

import { AppPropsContext } from '../App';
import { Device } from '../components/device/Device';
import Dialog from '../components/layout/Dialog';

interface StatItemProps {
	name: string;
	value: string;
	statistics_key: keyof Statistics;
	isLoading: boolean;
}

const StatItemNames: Record<string, string> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};

type OverviewState = {
	overviewStats: Record<keyof Statistics, { long: string; short: number }>;
	setOverviewStatsItem: (
		statName: keyof Statistics,
		statAmount: number | string,
		type: 'long' | 'short'
	) => void;
	setOverviewStats: (stats: Record<keyof Statistics, { long: string; short: number }>) => void;
};

export const useOverviewState = create<OverviewState>((set) => ({
	overviewStats: {} as Record<keyof Statistics, { long: string; short: number }>,
	setOverviewStatsItem: (statName, statAmount, type) =>
		set((state) => ({
			...state,
			overviewStats: {
				...state.overviewStats,
				[statName]: { ...state.overviewStats[statName], [type]: statAmount }
			}
		})),
	setOverviewStats(stats) {
		set((state) => ({
			...state,
			overviewStats: stats
		}));
	}
}));

const StatItem: React.FC<StatItemProps> = ({ name, statistics_key, value, isLoading }) => {
	const countUp = React.useRef(null);
	const appPropsContext = useContext(AppPropsContext);

	const [size, setSize] = React.useState<ByteSizeResult | null>(null);

	let amount = size ? +size.value : 0;

	const { overviewStats, setOverviewStatsItem } = useOverviewState();
	const shouldAnimate = amount !== 0 && overviewStats[statistics_key].short !== amount;

	useEffect(() => {
		setSize(byteSize(+value));
	}, [value]);

	useEffect(() => {
		console.log({
			name,
			stat: overviewStats[statistics_key],
			amount,
			shouldAnimate,
			amountsEqual: overviewStats[statistics_key].short === amount
		});
	}, [amount, overviewStats]);

	const { update: countTo } = useCountUp({
		ref: countUp,
		start: 0,
		end: 0,
		delay: 0.1,
		decimals: 1,
		duration: appPropsContext?.demoMode ? 1 : 0.5,
		useEasing: true
	});

	useEffect(() => {
		if (shouldAnimate) {
			console.log('updating to', amount);
			countTo(amount);
		}

		setOverviewStatsItem(statistics_key, amount, 'short');
	}, [amount]);

	return (
		<div
			className={clsx(
				'flex flex-col flex-shrink-0 w-32 px-4 py-3 duration-75 transform rounded-md cursor-default hover:bg-gray-50 hover:dark:bg-gray-600',
				!amount && 'hidden'
			)}
		>
			<span className="text-sm text-gray-400">{name}</span>
			<span className="text-2xl font-bold">
				{/* <span className="hidden" aria-hidden="true" ref={hiddenCountUp} /> */}
				{!isLoading ? (
					<div>
						<Skeleton enableAnimation={true} baseColor={'#21212e'} highlightColor={'#13131a'} />
						<span ref={countUp} hidden={true} />
					</div>
				) : (
					<span ref={countUp} />
				)}
				{!isLoading ? <></> : <span className="ml-1 text-[16px] text-gray-400">{size?.unit}</span>}
			</span>
			{/* {JSON.stringify(shouldAnimate)} */}
		</div>
	);
};

export const OverviewScreen = () => {
	const { data: libraryStatistics, isLoading: isStatisticsLoading } =
		useBridgeQuery('GetLibraryStatistics');
	const { data: nodeState } = useBridgeQuery('NodeGetState');

	const { overviewStats, setOverviewStats, setOverviewStatsItem } = useOverviewState();

	// get app props context
	const appPropsContext = useContext(AppPropsContext);

	useEffect(() => {
		if (appPropsContext?.demoMode == true && !libraryStatistics?.library_db_size) {
			if (!Object.entries(overviewStats).length)
				setOverviewStats({
					total_bytes_capacity: { long: '8093333345230', short: 0 },
					preview_media_bytes: { long: '2304387532', short: 0 },
					library_db_size: { long: '83345230', short: 0 },
					total_file_count: { long: '20342345', short: 0 },
					total_bytes_free: { long: '89734502034', short: 0 },
					total_bytes_used: { long: '8093333345230', short: 0 },
					total_unique_bytes: { long: '9347397', short: 0 }
				});
		} else {
			const newStatistics = {
				total_bytes_capacity: { long: '', short: 0 },
				preview_media_bytes: { long: '', short: 0 },
				library_db_size: { long: '', short: 0 },
				total_file_count: { long: '', short: 0 },
				total_bytes_free: { long: '', short: 0 },
				total_bytes_used: { long: '', short: 0 },
				total_unique_bytes: { long: '', short: 0 }
			};

			Object.entries(libraryStatistics as Statistics).map(([key, value]) => {
				newStatistics[key as keyof Statistics] = { long: value as string, short: 0 };
			});

			setOverviewStats(newStatistics);
		}
	}, [appPropsContext, libraryStatistics]);

	useEffect(() => {
		setTimeout(() => {
			setOverviewStatsItem('total_bytes_capacity', '4093333345230', 'long');
		}, 10000);
	}, [overviewStats]);

	const validStatItems = Object.keys(StatItemNames);

	return (
		<div className="flex flex-col w-full h-screen overflow-x-hidden custom-scroll page-scroll">
			<div data-tauri-drag-region className="flex flex-shrink-0 w-full h-5" />
			{/* PAGE */}
			<div className="flex flex-col w-full h-screen px-3">
				{/* STAT HEADER */}
				<div className="flex w-full">
					{/* STAT CONTAINER */}
					<div className="flex pb-4 overflow-hidden">
						{Object.entries(overviewStats).map(([key, value]) => {
							if (!validStatItems.includes(key)) return <></>;

							return (
								<StatItem
									key={key}
									name={StatItemNames[key]}
									value={value.long}
									statistics_key={key as keyof Statistics}
									isLoading={isStatisticsLoading}
								/>
							);
						})}
					</div>
					<div className="flex-grow" />
					<div className="space-x-2">
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
				<div className="flex flex-col pb-4 space-y-4">
					{nodeState && (
						<Device
							name={nodeState?.node_name ?? 'This Device'}
							size="1.4TB"
							runningJob={{ amount: 65, task: 'Generating preview media' }}
							locations={[
								{ name: 'Pictures', folder: true },
								{ name: 'Downloads', folder: true },
								{ name: 'Minecraft', folder: true }
							]}
							type="laptop"
						/>
					)}
					<Device
						name={`James' iPhone 12`}
						size="47.7GB"
						locations={[
							{ name: 'Camera Roll', folder: true },
							{ name: 'Notes', folder: true },
							{ name: 'App.tsx', format: 'tsx', icon: 'reactts' },
							{ name: 'vite.config.js', format: 'js', icon: 'vite' }
						]}
						type="phone"
					/>
					<Device
						name={`Spacedrive Server`}
						size="5GB"
						locations={[
							{ name: 'Cached', folder: true },
							{ name: 'Photos', folder: true },
							{ name: 'Documents', folder: true }
						]}
						type="server"
					/>
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
