import { IconName } from '@sd/assets/util';
import { useEffect, useMemo, useState } from 'react';
import { humanizeSize } from '@sd/client';
import { Card, CircularProgress, tw } from '@sd/ui';
import { Icon } from '~/components';
import { useIsDark, useLocale } from '~/hooks';

type StatCardProps = {
	name: string;
	icon: IconName;
	totalSpace: string | number[];
	freeSpace?: string | number[];
	color: string;
	connectionType: 'lan' | 'p2p' | 'cloud' | null;
};

const NBSP = '\xa0';

const Pill = tw.div`px-1.5 py-[1px] rounded text-tiny font-medium text-ink-dull bg-app-box border border-app-line font-plex font-medium tracking-wide`;

const StatCard = ({ icon, name, connectionType, ...stats }: StatCardProps) => {
	const [mounted, setMounted] = useState(false);

	const isDark = useIsDark();

	//TODO: Improve this
	const totalSpaceSingleValue = humanizeSize(stats.totalSpace);

	const { totalSpace, freeSpace, usedSpaceSpace } = useMemo(() => {
		const totalSpace = humanizeSize(stats.totalSpace);
		const freeSpace = stats.freeSpace == null ? totalSpace : humanizeSize(stats.freeSpace);

		return {
			totalSpace,
			freeSpace,
			usedSpaceSpace: humanizeSize(totalSpace.bytes - freeSpace.bytes)
		};
	}, [stats]);

	const progress = useMemo(() => {
		if (!mounted || totalSpace.bytes === 0n) return 0;
		// Calculate progress using raw bytes to avoid unit conversion issues
		return Math.floor((Number(usedSpaceSpace.bytes) / Number(totalSpace.bytes)) * 100);
	}, [mounted, totalSpace, usedSpaceSpace]);

	useEffect(() => {
		setMounted(true);
	}, []);

	const { t } = useLocale();

	return (
		<Card className="flex w-[280px] shrink-0 flex-col bg-app-box/50 !p-0">
			<div className="flex flex-row items-center gap-5 p-4">
				{stats.freeSpace && (
					<CircularProgress
						radius={40}
						progress={progress}
						strokeWidth={6}
						trackStrokeWidth={6}
						strokeColor={
							progress >= 90
								? '#E14444'
								: progress >= 75
									? 'darkorange'
									: progress >= 60
										? 'yellow'
										: '#2599FF'
						}
						fillColor="transparent"
						trackStrokeColor={isDark ? '#252631' : '#efefef'}
						strokeLinecap="square"
						className="flex items-center justify-center"
						transition="stroke-dashoffset 1s ease 0s, stroke 1s ease"
					>
						<div className="absolute text-lg font-semibold">
							{usedSpaceSpace.value}
							<span className="ml-0.5 text-tiny opacity-60">
								{t(`size_${usedSpaceSpace.unit.toLowerCase()}`)}
							</span>
						</div>
					</CircularProgress>
				)}
				<div className="flex flex-col overflow-hidden">
					<Icon className="-ml-1 -mt-1 min-h-[60px] min-w-[60px]" name={icon} size={60} />
					<span className="mt-2 truncate font-plex font-bold">{name}</span>
					<span className="mt-0 whitespace-pre font-plex text-tiny font-semibold tracking-wide text-ink-faint">
						{freeSpace.value !== totalSpace.value && (
							<>
								{freeSpace.value} {t(`size_${freeSpace.unit.toLowerCase()}`)}{' '}
								{t('free_of')} {totalSpaceSingleValue.value}{' '}
								{t(`size_${totalSpace.unit.toLowerCase()}`)}
							</>
						)}
					</span>
				</div>
			</div>
			<div className="flex h-10 flex-row items-center gap-1.5 border-t border-app-line px-2">
				{freeSpace.value === totalSpace.value && (
					<Pill>
						{totalSpace.value} {t(`size_${totalSpace.unit.toLowerCase()}`)}
					</Pill>
				)}
				<Pill className="uppercase">{connectionType || t('local')}</Pill>
				<div className="grow" />
				{/* <Button size="icon" variant="outline">
					<Ellipsis className="w-3 h-3 opacity-50" />
				</Button> */}
			</div>
		</Card>
	);
};

export default StatCard;
