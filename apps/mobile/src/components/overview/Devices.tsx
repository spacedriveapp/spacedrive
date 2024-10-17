import * as RNFS from '@dr.pogodin/react-native-fs';
import { RSPCError } from '@spacedrive/rspc-client';
import { UseQueryResult } from '@tanstack/react-query';
import React, { useEffect, useState } from 'react';
import { Platform, Text, View } from 'react-native';
import DeviceInfo from 'react-native-device-info';
import { ScrollView } from 'react-native-gesture-handler';

import { HardwareModel, NodeState, StatisticsResponse } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import Fade from '../layout/Fade';
import { Button } from '../primitive/Button';
import NewCard from './NewCard';
import OverviewSection from './OverviewSection';
import StatCard from './StatCard';

interface Props {
	node: NodeState | undefined;
	stats: UseQueryResult<StatisticsResponse, RSPCError>;
}

export function hardwareModelToIcon(hardwareModel: HardwareModel) {
	switch (hardwareModel) {
		case 'MacBookPro':
			return 'Laptop';
		case 'MacStudio':
			return 'SilverBox';
		case 'IPhone':
			return 'Mobile';
		case 'IPad':
			return 'Tablet';
		case 'Simulator':
			return 'Drive';
		case 'Android':
			return 'Mobile';
		default:
			return 'Laptop';
	}
}

const Devices = ({ node, stats }: Props) => {
	// We don't need the totalSpaceEx and freeSpaceEx fields
	const [sizeInfo, setSizeInfo] = useState<
		Omit<RNFS.FSInfoResultT, 'totalSpaceEx' | 'freeSpaceEx'>
	>({ freeSpace: 0, totalSpace: 0 });
	const [deviceName, setDeviceName] = useState<string>('');

	useEffect(() => {
		const getFSInfo = async () => {
			return await RNFS.getFSInfo();
		};
		getFSInfo().then(size => {
			setSizeInfo(size);
		});
	}, []);

	const totalSpace =
		Platform.OS === 'android'
			? sizeInfo.totalSpace.toString()
			: stats.data?.statistics?.total_local_bytes_capacity || '0';
	const freeSpace =
		Platform.OS === 'android'
			? sizeInfo.freeSpace.toString()
			: stats.data?.statistics?.total_local_bytes_free || '0';

	useEffect(() => {
		if (Platform.OS === 'android') {
			DeviceInfo.getDeviceName().then(name => {
				setDeviceName(name);
			});
		} else if (node) {
			setDeviceName(node.name);
		}
	}, [node]);

	return (
		<OverviewSection title="Devices" count={node ? 1 : 0}>
			<View>
				<Fade height={'100%'} width={30} color="black">
					<ScrollView
						horizontal
						showsHorizontalScrollIndicator={false}
						contentContainerStyle={tw`px-6`}
					>
						{node && (
							<StatCard
								name={deviceName}
								// TODO (Optional): Use Brand Type for Different Android Models/iOS Models using DeviceInfo.getBrand()
								icon={hardwareModelToIcon(node.device_model as any)}
								totalSpace={totalSpace}
								freeSpace={freeSpace}
								color="#0362FF"
								connectionType={null}
							/>
						)}
						<NewCard
							icons={['Laptop', 'Server', 'SilverBox', 'Tablet']}
							text="Spacedrive works best on all your devices."
							style={twStyle(node ? 'ml-2' : 'ml-0')}
							button={() => (
								<Button variant="transparent">
									<Text style={tw`font-bold text-ink-dull`}>Coming soon</Text>
								</Button>
							)}
						/>
					</ScrollView>
				</Fade>
			</View>
		</OverviewSection>
	);
};

export default Devices;
