import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { useEffect, useMemo, useState } from 'react';
import { byteSize } from '@sd/client';
import { Button, Card, CircularProgress, tw } from '@sd/ui';
import { Icon } from '~/components';
import { useIsDark } from '~/hooks';

type StatCardProps = {
	name: string;
	icon: string;
	totalSpace: string | number[];
	freeSpace?: string | number[];
	color: string;
	connectionType: 'lan' | 'p2p' | 'cloud' | null;
};

const Pill = tw.div`px-1.5 py-[1px] rounded text-tiny font-medium text-ink-dull bg-app-box border border-app-line`;

const StatCard = ({ icon, name, connectionType, ...stats }: StatCardProps) => {
	const [mounted, setMounted] = useState(false);

	const isDark = useIsDark();

	const { totalSpace, freeSpace, usedSpaceSpace } = useMemo(() => {
		const totalSpace = byteSize(stats.totalSpace)
		const freeSpace = stats.freeSpace == null ? totalSpace : byteSize(stats.freeSpace)
		return {
			totalSpace,
			freeSpace,
			usedSpaceSpace: byteSize(totalSpace.original - freeSpace.original)
		};
	}, [stats]);

	useEffect(() => {
		setMounted(true);
	}, []);

	const progress = useMemo(() => {
		if (!mounted || totalSpace.original === 0n) return 0;
		return Math.floor((usedSpaceSpace.value / totalSpace.value) * 100);
	}, [mounted, totalSpace, usedSpaceSpace]);

	return (
		<Card className="flex w-[280px] shrink-0 flex-col  bg-app-box/50 !p-0 ">
			<div className="flex flex-row items-center gap-5 p-4 px-6 ">
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
							{usedSpaceSpace.value}
							<span className="ml-0.5 text-tiny opacity-60">
								{usedSpaceSpace.unit}
							</span>
						</div>
					</CircularProgress>
				)}
				<div className="flex flex-col overflow-hidden">
					<Icon className="-ml-1" name={icon as any} size={60} />
					<span className="truncate font-medium">{name}</span>
					<span className="mt-1 truncate text-tiny text-ink-faint">
						{freeSpace.value}
						{freeSpace.unit} free of {totalSpace.value}
						{totalSpace.unit}
					</span>
				</div>
			</div>
			<div className="flex h-10 flex-row items-center gap-1.5  border-t border-app-line px-2">
				<Pill className="uppercase">{connectionType || 'Local'}</Pill>
				<div className="grow" />
				{/* <Button size="icon" variant="outline">
					<Ellipsis className="h-3 w-3 opacity-50" />
				</Button> */}
			</div>
		</Card>
	);
};

export default StatCard;
