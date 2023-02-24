import { PropsWithChildren } from 'react';
import { FlatList } from 'react-native';

export default function VirtualizedListWrapper({ children }: PropsWithChildren) {
	return (
		<FlatList
			data={[]}
			keyExtractor={() => 'key'}
			showsHorizontalScrollIndicator={false}
			showsVerticalScrollIndicator={false}
			renderItem={null}
			ListHeaderComponent={<>{children}</>}
		/>
	);
}
