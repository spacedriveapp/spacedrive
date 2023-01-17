import { View } from 'react-native';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import OverviewStats from '~/containers/OverviewStats';
import tw from '~/lib/tailwind';
import { OverviewStackScreenProps } from '~/navigation/tabs/OverviewStack';

export default function OverviewScreen({ navigation }: OverviewStackScreenProps<'Overview'>) {
	return (
		<VirtualizedListWrapper>
			<View style={tw`px-4 mt-4`}>
				{/* Stats */}
				<OverviewStats />
			</View>
		</VirtualizedListWrapper>
	);
}
