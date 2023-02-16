import { View } from 'react-native';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import OverviewStats from '~/components/overview/OverviewStats';
import tw from '~/lib/tailwind';
import { OverviewStackScreenProps } from '~/navigation/tabs/OverviewStack';

export default function OverviewScreen({ navigation }: OverviewStackScreenProps<'Overview'>) {
	return (
		<VirtualizedListWrapper>
			<View style={tw`mt-4 px-4`}>
				{/* Stats */}
				<OverviewStats />
			</View>
		</VirtualizedListWrapper>
	);
}
