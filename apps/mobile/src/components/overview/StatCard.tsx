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
};

const StatCard = ({ icon, name, connectionType, ...stats }: StatCardProps) => {
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
		return Math.floor((usedSpaceSpace.value / totalSpace.value) * 100);
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
							fill={usedSpaceSpace.value}
							lineCap="round"
							backgroundColor={tw.color('ink-light/5')}
							tintColor={progress > 90 ? '#E14444' : '#2599FF'}
							style={tw`flex items-center justify-center`}
						>
							{(fill) => (
								<View
									style={tw`absolute flex-row items-end gap-0.5 text-lg font-semibold`}
								>
									<Text style={tw`mx-auto text-md font-semibold text-ink`}>
										{fill.toFixed(0)}
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
					<Text numberOfLines={1} style={tw`max-w-[130px] py-1 font-medium text-ink`}>
						{name}
					</Text>
					<Text numberOfLines={1} style={tw`max-w-[130px] text-ink-faint`}>
						{freeSpace.value}
						{freeSpace.unit} free of {totalSpace.value}
						{totalSpace.unit}
					</Text>
				</View>
			</View>
			<View
				style={tw`flex h-10 flex-row items-center gap-1.5  border-t border-app-cardborder px-2`}
			>
				<View
					style={tw`rounded border border-app-lightborder bg-app-highlight px-1.5 py-px`}
				>
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
