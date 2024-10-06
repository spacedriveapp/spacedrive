import { Info } from '@phosphor-icons/react';
import { getIcon } from '@sd/assets/util';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import React, { MouseEventHandler, useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import {
	KindStatistic,
	uint32ArrayToBigInt,
	useLibraryQuery,
	useLibrarySubscription
} from '@sd/client';
import { Card, Loader, Tooltip } from '@sd/ui';
import { useIsDark, useLocale } from '~/hooks';

import { FileKind } from '.';

const INFO_ICON_CLASSLIST =
	'inline size-3 text-ink-faint opacity-0 ml-1 transition-opacity duration-300 group-hover:opacity-70';
const TOTAL_FILES_CLASSLIST =
	'flex items-center justify-between whitespace-nowrap text-sm font-medium text-ink-dull mt-2 px-1 font-plex';
const UNIDENTIFIED_FILES_CLASSLIST = 'relative flex items-center text-xs font-plex text-ink-faint';
const BARS_CONTAINER_CLASSLIST =
	'relative mx-2.5 grid grow grid-cols-[repeat(auto-fit,_minmax(0,_1fr))] grid-rows-[136px_12px] font-plex tracking-wide items-end justify-items-center gap-x-1.5 gap-y-1 self-stretch';

const mapFractionalValue = (numerator: bigint, denominator: bigint, maxValue: bigint): string => {
	if (denominator === 0n) return '0';
	const result = (numerator * maxValue) / denominator;
	if (numerator !== 0n && result < 1n) return '1';
	return result.toString();
};

const formatNumberWithCommas = (number: number | bigint) => number.toLocaleString();

const interpolateHexColor = (color1: string, color2: string, factor: number): string => {
	const hex = (color: string) => parseInt(color.slice(1), 16);
	const r = Math.round((1 - factor) * (hex(color1) >> 16) + factor * (hex(color2) >> 16));
	const g = Math.round(
		(1 - factor) * ((hex(color1) >> 8) & 0x00ff) + factor * ((hex(color2) >> 8) & 0x00ff)
	);
	const b = Math.round(
		(1 - factor) * (hex(color1) & 0x0000ff) + factor * (hex(color2) & 0x0000ff)
	);
	return `#${((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1).toUpperCase()}`;
};

const defaultFileKinds: FileKind[] = [
	{ kind: 4, name: 'Package', count: 0n, total_bytes: 0n },
	{ kind: 8, name: 'Archive', count: 0n, total_bytes: 0n },
	{ kind: 9, name: 'Executable', count: 0n, total_bytes: 0n },
	{ kind: 11, name: 'Encrypted', count: 0n, total_bytes: 0n },
	{ kind: 12, name: 'Key', count: 0n, total_bytes: 0n },
	{ kind: 13, name: 'Link', count: 0n, total_bytes: 0n },
	{ kind: 14, name: 'WebPageArchive', count: 0n, total_bytes: 0n },
	{ kind: 15, name: 'Widget', count: 0n, total_bytes: 0n },
	{ kind: 16, name: 'Album', count: 0n, total_bytes: 0n },
	{ kind: 17, name: 'Collection', count: 0n, total_bytes: 0n },
	{ kind: 18, name: 'Font', count: 0n, total_bytes: 0n },
	{ kind: 19, name: 'Mesh', count: 0n, total_bytes: 0n },
	{ kind: 20, name: 'Code', count: 0n, total_bytes: 0n },
	{ kind: 21, name: 'Database', count: 0n, total_bytes: 0n },
	{ kind: 22, name: 'Book', count: 0n, total_bytes: 0n },
	{ kind: 23, name: 'Config', count: 0n, total_bytes: 0n },
	{ kind: 24, name: 'Dotfile', count: 0n, total_bytes: 0n },
	{ kind: 25, name: 'Screenshot', count: 0n, total_bytes: 0n }
];

const FileKindStats: React.FC = () => {
	const isDark = useIsDark();
	const navigate = useNavigate();
	const { t } = useLocale();
	const { data } = useLibraryQuery(['library.kindStatistics']);
	const [fileKinds, setFileKinds] = useState<Map<number, FileKind>>(new Map());
	const [cardWidth, setCardWidth] = useState<number>(0);
	const [loading, setLoading] = useState<boolean>(true);
	const containerRef = useRef<HTMLDivElement>(null);
	const iconsRef = useRef<{ [key: string]: HTMLImageElement }>({});

	const BAR_MAX_HEIGHT = 115n;
	const BAR_COLOR_START = '#36A3FF';
	const BAR_COLOR_END = '#004C99';

	useLibrarySubscription(['library.updatedKindStatistic'], {
		onData: (data: KindStatistic) => {
			setFileKinds((kindStatisticsMap) => {
				if (uint32ArrayToBigInt(data.count) !== 0n) {
					return new Map(
						kindStatisticsMap.set(data.kind, {
							kind: data.kind,
							name: data.name,
							count: uint32ArrayToBigInt(data.count),
							total_bytes: uint32ArrayToBigInt(data.total_bytes)
						})
					);
				}

				return kindStatisticsMap;
			});
		}
	});

	const formatCount = (count: number | bigint): string => {
		const bigIntCount = typeof count === 'number' ? BigInt(count) : count;

		return bigIntCount >= 1000n ? `${bigIntCount / 1000n}K` : count.toString();
	};

	const handleResize = useCallback(() => {
		if (containerRef.current) {
			const factor = window.innerWidth > 1500 ? 0.35 : 0.4;
			setCardWidth(window.innerWidth * factor);
		}
	}, []);

	useEffect(() => {
		window.addEventListener('resize', handleResize);
		handleResize();

		const containerElement = containerRef.current;
		if (containerElement) {
			const observer = new MutationObserver(handleResize);
			observer.observe(containerElement, {
				attributes: true,
				childList: true,
				subtree: true,
				attributeFilter: ['style']
			});

			return () => {
				observer.disconnect();
			};
		}

		return () => {
			window.removeEventListener('resize', handleResize);
		};
	}, [handleResize, fileKinds]);

	useEffect(() => {
		if (data) {
			const statistics = new Map(
				Object.entries(data.statistics)
					.filter(([_, stats]) => uint32ArrayToBigInt(stats.count) !== 0n)
					.sort(([_keyA, a], [_keyB, b]) => {
						const aCount = uint32ArrayToBigInt(a.count);
						const bCount = uint32ArrayToBigInt(b.count);
						if (aCount === bCount) return 0;
						return aCount > bCount ? -1 : 1;
					})
					.map(([_, stats]) => [
						stats.kind,
						{
							kind: stats.kind,
							name: stats.name,
							count: uint32ArrayToBigInt(stats.count),
							total_bytes: uint32ArrayToBigInt(stats.total_bytes)
						}
					])
			);

			if (statistics.size < 10) {
				const additionalKinds = defaultFileKinds.filter(
					(defaultKind) => !statistics.has(defaultKind.kind)
				);
				const kindsToAdd = additionalKinds.slice(0, 10 - statistics.size);

				for (const kindToAdd of kindsToAdd) {
					statistics.set(kindToAdd.kind, kindToAdd);
				}
			}

			setFileKinds(statistics);

			Object.values(data.statistics).forEach((item: { name: string }) => {
				const iconName = item.name;
				if (!iconsRef.current[iconName]) {
					const img = new Image();
					img.src = getIcon(iconName + '20', isDark);
					iconsRef.current[iconName] = img;
				}
			});

			setLoading(false);
		}
	}, [data, isDark]);

	const sortedFileKinds = [...fileKinds.values()].sort((a, b) => {
		if (a.count === b.count) return 0;
		return a.count > b.count ? -1 : 1;
	});

	const maxFileCount = sortedFileKinds && sortedFileKinds[0] ? sortedFileKinds[0].count : 0n;
	const barCount = sortedFileKinds.length;
	const makeBarClickHandler =
		(fileKind: FileKind): MouseEventHandler<HTMLDivElement> | undefined =>
		() => {
			const path = {
				pathname: '../search',
				search: new URLSearchParams({
					filters: JSON.stringify([{ object: { kind: { in: [fileKind.kind] } } }])
				}).toString()
			};
			navigate(path);
		};

	return (
		<div className="flex justify-center tabular-nums">
			<Card
				ref={containerRef}
				className="max-w-1/2 group mx-1 flex h-[220px] w-full min-w-[400px] shrink-0 flex-col gap-2 bg-app-box/50"
			>
				{loading ? (
					<div className="mt-4 flex h-full items-center justify-center">
						<div className="flex flex-col items-center justify-center gap-3">
							<Loader />
							<p className="text-ink-dull">{t('fetching_file_kind_statistics')}</p>
						</div>
					</div>
				) : (
					<>
						<div className={TOTAL_FILES_CLASSLIST}>
							<Tooltip className="flex items-center" label={t('bar_graph_info')}>
								<div className="flex items-center gap-2">
									<span
										className={clsx(
											'text-xl font-black',
											isDark ? 'text-white' : 'text-black'
										)}
									>
										{data?.total_identified_files
											? formatNumberWithCommas(data.total_identified_files)
											: '0'}{' '}
									</span>
									<div className="flex items-center">
										{t('total_files')}
										<Info weight="fill" className={INFO_ICON_CLASSLIST} />
									</div>
								</div>
							</Tooltip>
							<div className={UNIDENTIFIED_FILES_CLASSLIST}>
								<Tooltip label={t('unidentified_files_info')}>
									<span>
										{data?.total_unidentified_files
											? formatNumberWithCommas(data.total_unidentified_files)
											: '0'}{' '}
										{t('unidentified_files')}
									</span>
								</Tooltip>
							</div>
						</div>
						<div className={BARS_CONTAINER_CLASSLIST}>
							{sortedFileKinds.map((fileKind, index) => {
								const iconImage = iconsRef.current[fileKind.name];
								const barColor = interpolateHexColor(
									BAR_COLOR_START,
									BAR_COLOR_END,
									index / (barCount - 1)
								);

								const barHeight =
									mapFractionalValue(
										fileKind.count,
										maxFileCount,
										BAR_MAX_HEIGHT
									) + 'px';

								return (
									<>
										<Tooltip
											asChild
											key={fileKind.kind}
											label={
												formatNumberWithCommas(fileKind.count) +
												' ' +
												t(fileKind.name.toLowerCase(), {
													count: Number(fileKind.count)
												})
											}
											position="left"
										>
											<div
												className="relative flex w-full min-w-8 max-w-10 grow cursor-pointer flex-col items-center"
												onDoubleClick={makeBarClickHandler(fileKind)}
											>
												{iconImage && (
													<img
														src={iconImage.src}
														alt={fileKind.name}
														className="relative mb-1 size-4 duration-500"
													/>
												)}
												<motion.div
													className="flex w-full flex-col items-center rounded transition-all duration-500"
													initial={{ height: 0 }}
													animate={{ height: barHeight }}
													transition={{
														duration: 0.4,
														ease: [0.42, 0, 0.58, 1]
													}}
													style={{
														backgroundColor: barColor
													}}
												></motion.div>
											</div>
										</Tooltip>
										<div className="sm col-span-1 row-start-2 row-end-auto text-center text-[10px] font-medium text-ink-faint">
											{formatCount(fileKind.count)}
										</div>
									</>
								);
							})}
						</div>
					</>
				)}
			</Card>
		</div>
	);
};

export default FileKindStats;
