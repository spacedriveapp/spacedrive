import { useEffect, useMemo, useState } from 'react';
import { Text, View } from 'react-native';
import { AnimatedCircularProgress } from 'react-native-circular-progress';
import { humanizeSize } from '@sd/client';
import { tw } from '~/lib/tailwind';

import { Icon, IconName } from '../icons/Icon';
import Card from '../layout/Card';

type StatCardProps = {
	name: string;
	icon: IconName;
	totalSpace: string | number[];
	freeSpace?: string | number[];
	color: string;
	connectionType: 'lan' | 'p2p' | 'cloud' | null;
	type?: 'location' | 'device'; //for layout purposes
};

const infoBox = tw`rounded border border-app-lightborder/50 bg-app-highlight/50 px-1.5 py-px`;

const StatCard = ({ icon, name, connectionType, type, ...stats }: StatCardProps) => {
	const [mounted, setMounted] = useState(false);

	const { totalSpace, freeSpace, usedSpaceSpace } = useMemo(() => {
		const totalSpace = humanizeSize(stats.totalSpace);
		const freeSpace = stats.freeSpace == null ? totalSpace : humanizeSize(stats.freeSpace);
		return {
			totalSpace,
			freeSpace,
			usedSpaceSpace: humanizeSize(totalSpace.bytes - freeSpace.bytes)
		};
	}, [stats]);

	useEffect(() => {
		setMounted(true);
	}, []);

	const progress = useMemo(() => {
		if (!mounted || totalSpace.bytes === 0n) return 0;
		// Calculate progress using raw bytes to avoid unit conversion issues
		return Math.floor((Number(usedSpaceSpace.bytes) / Number(totalSpace.bytes)) * 100);
	}, [mounted, totalSpace, usedSpaceSpace]);

	return (
		<Card style={tw`w-[300px] shrink-0 flex-col p-0`}>
			<View style={tw`flex flex-row items-center gap-5 p-4 px-6`}>
				{stats.freeSpace && (
					<>
						<AnimatedCircularProgress
							size={90}
							width={7}
							rotation={0}
							fill={progress}
							lineCap="round"
							backgroundColor={tw.color('ink-light/5')}
							tintColor={
								progress >= 90
									? '#E14444'
									: progress >= 75
										? 'darkorange'
										: progress >= 60
											? 'yellow'
											: '#2599FF'
							}
							style={tw`flex items-center justify-center`}
						>
							{() => (
								<View
									style={tw`absolute flex-row items-end gap-0.5 text-lg font-semibold`}
								>
									<Text style={tw`mx-auto text-md font-semibold text-ink`}>
										{usedSpaceSpace.value}
									</Text>
									<Text style={tw`text-xs font-bold text-ink-dull opacity-60`}>
										{usedSpaceSpace.unit}
									</Text>
								</View>
							)}
						</AnimatedCircularProgress>
					</>
				)}
				<View style={tw`flex-col overflow-hidden`}>
					<Icon style={tw`-ml-1`} name={icon} size={60} />
					<Text numberOfLines={1} style={tw`max-w-[150px] py-1 font-medium text-ink`}>
						{name}
					</Text>
					{type !== 'location' && (
						<Text numberOfLines={1} style={tw`max-w-[150px] text-xs text-ink-faint`}>
							{freeSpace.value}
							{freeSpace.unit} free of {totalSpace.value}
							{totalSpace.unit}
						</Text>
					)}
				</View>
			</View>
			<View
				style={tw`flex h-10 flex-row items-center gap-1.5  border-t border-app-cardborder px-2`}
			>
				{type === 'location' && (
					<View style={infoBox}>
						<Text style={tw`text-xs font-medium uppercase text-ink-dull`}>
							{totalSpace.value}
							{totalSpace.unit}
						</Text>
					</View>
				)}
				<View style={infoBox}>
					<Text style={tw`text-xs font-medium uppercase text-ink-dull`}>
						{connectionType || 'Local'}
					</Text>
				</View>
				<View style={tw`grow`} />
			</View>
		</Card>
	);
};

export default StatCard;
