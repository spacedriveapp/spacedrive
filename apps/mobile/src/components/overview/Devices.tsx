import { AlphaRSPCError } from '@oscartbeaumont-sd/rspc-client/v2';
import { UseQueryResult } from '@tanstack/react-query';
import React from 'react';
import { Text, View } from 'react-native';
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
	stats: UseQueryResult<StatisticsResponse, AlphaRSPCError>;
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
	return (
			<OverviewSection title="Devices" count={node ? 1 : 0}>
				<View>
					<Fade height={'100%'} width={30} color="mobile-screen">
						<ScrollView
							horizontal
							showsHorizontalScrollIndicator={false}
							contentContainerStyle={tw`px-6`}
						>
							{node && (
								<StatCard
									name={node.name}
									icon={hardwareModelToIcon(node.device_model as any)}
									totalSpace={stats.data?.statistics?.total_bytes_capacity || '0'}
									freeSpace={stats.data?.statistics?.total_bytes_free || '0'}
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
