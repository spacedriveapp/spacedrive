import { ReactNode } from 'react';
import { FlatList, FlatListProps } from 'react-native';

type Props = {
	children: ReactNode;
} & Partial<FlatListProps<unknown>>;

export default function VirtualizedListWrapper({ children, ...rest }: Props) {
	return (
		<FlatList
			{...rest}
			data={[]}
			keyExtractor={() => 'key'}
			showsHorizontalScrollIndicator={false}
			showsVerticalScrollIndicator={false}
			renderItem={null}
			ListHeaderComponent={<>{children}</>}
		/>
	);
}
