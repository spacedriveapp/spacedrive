import { useEffect, useMemo, useState } from 'react';
import { Text, View } from 'react-native';
import { AnimatedCircularProgress } from 'react-native-circular-progress';
import { byteSize } from '@sd/client';
import { tw } from '~/lib/tailwind';

import { Icon } from '../icons/Icon';

type StatCardProps = {
	name: string;
	icon: string;
	totalSpace: string | number[];
	freeSpace?: string | number[];
	color: string;
	connectionType: 'lan' | 'p2p' | 'cloud' | null;
};

const StatCard = ({ icon, name, connectionType, ...stats }: StatCardProps) => {
	const [mounted, setMounted] = useState(false);

	const { totalSpace, freeSpace, usedSpaceSpace } = useMemo(() => {
		const totalSpace = byteSize(stats.totalSpace);
		const freeSpace = stats.freeSpace == null ? totalSpace : byteSize(stats.freeSpace);
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

	console.log(usedSpaceSpace.value);

	return (
		<View
			style={tw`flex w-[300px] shrink-0 flex-col rounded-md border border-app-line/50 bg-app-box/50`}
		>
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
					<Icon style={tw`-ml-1`} name={icon as any} size={60} />
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
				style={tw`flex h-10 flex-row items-center gap-1.5  border-t border-app-line px-2`}
			>
				<View style={tw`rounded border border-app-line bg-app-box px-1.5 py-[1px]`}>
					<Text style={tw`text-xs font-medium text-ink-dull`}>
						{connectionType || 'Local'}
					</Text>
				</View>
				<View style={tw`grow`} />
			</View>
		</View>
	);
};

export default StatCard;
