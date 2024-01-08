import { useBottomTabBarHeight } from '@react-navigation/bottom-tabs';
import { View } from 'react-native';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import OverviewStats from '~/components/overview/OverviewStats';
import { twStyle } from '~/lib/tailwind';
import { OverviewStackScreenProps } from '~/navigation/tabs/OverviewStack';

export default function OverviewScreen({ navigation }: OverviewStackScreenProps<'Overview'>) {
	const height = useBottomTabBarHeight();

	return (
		<VirtualizedListWrapper>
			<View style={twStyle('mt-4 px-4', { marginBottom: height })}>
				<OverviewStats />
			</View>
		</VirtualizedListWrapper>
	);
}
