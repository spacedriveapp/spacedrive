import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { useEffect, useMemo, useState } from 'react';
import { humanizeSize } from '@sd/client';
import { Button, Card, CircularProgress, tw } from '@sd/ui';
import { Icon } from '~/components';
import { useIsDark, useLocale } from '~/hooks';

type StatCardProps = {
	name: string;
	icon: string;
	totalSpace: string | number[];
	freeSpace?: string | number[];
	color: string;
	connectionType: 'lan' | 'p2p' | 'cloud' | null;
	devices: boolean;
};

const Pill = tw.div`px-1.5 py-[1px] rounded text-tiny font-medium text-ink-dull bg-app-box border border-app-line`;

const StatCard = ({ icon, name, devices, connectionType, ...stats }: StatCardProps) => {
	const [mounted, setMounted] = useState(false);


	const isDark = useIsDark();

	const { totalSpace, freeSpace, usedSpace } = useMemo(() => {
		const totalSpace = humanizeSize(stats.totalSpace);

		const freeSpace = stats.freeSpace == null ? totalSpace : humanizeSize(stats.freeSpace);

		const usedSpaceCalculation = humanizeSize(totalSpace.value - freeSpace.value);

		return {
			totalSpace,
			freeSpace,
			usedSpace: usedSpaceCalculation,
		};

	}, [stats]);
	useEffect(() => {
		setMounted(true);
	}, []);

	usedSpace.unit = humanizeSize(Number(stats.totalSpace) - Number(stats.freeSpace)).unit;

	const progress = useMemo(() => {
		if (!mounted || totalSpace.original === 0) return 0;
		return Math.floor((usedSpace.value / totalSpace.value) * 100);
	}, [mounted, totalSpace, usedSpace]);

	const { t } = useLocale();
	return (
		<Card className="flex w-[280px] shrink-0 flex-col  bg-app-box/50 !p-0 ">
			<div className="max-h-28 flex flex-row items-center gap-5 p-4 px-6">
				{stats.freeSpace && (
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
							{usedSpace.value}
							<span className="ml-0.5 text-tiny opacity-60">
								{usedSpace.unit}
							</span>
						</div>
					</CircularProgress>
				)}
				<div className="flex flex-col overflow-hidden">
					<Icon className="-ml-1" name={icon as any} size={60} />
					<span className="truncate font-medium">{name}</span>
					<span className="mt-1 truncate text-tiny text-ink-faint">
						{freeSpace.value}
						{freeSpace.unit} {devices && t('free_of') + " " + totalSpace.value + totalSpace.unit}
					</span>
				</div>
			</div>
			<div className="flex h-10 flex-row items-center gap-1.5 border-t border-app-line px-2">
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
