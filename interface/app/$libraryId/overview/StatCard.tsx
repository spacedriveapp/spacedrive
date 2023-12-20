import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { useEffect, useMemo, useState } from 'react';
import { byteSize } from '@sd/client';
import { Button, Card, CircularProgress, tw } from '@sd/ui';
import { Icon } from '~/components';
import { useIsDark } from '~/hooks';

type StatCardProps = {
	name: string;
	icon: string;
	total_space: string;
	free_space: string;
	color: string;
	connection_type: 'lan' | 'p2p' | 'cloud';
};

const Pill = tw.div`px-1.5 py-[1px] rounded text-tiny font-medium text-ink-dull bg-app-box border border-app-line`;

const StatCard = ({ icon, name, connection_type, ...stats }: StatCardProps) => {
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
		<Card className="flex w-[280px] shrink-0 flex-col  bg-app-box/50 !p-0 ">
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
					<Icon name={icon as any} size={60} />
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

export default StatCard;
