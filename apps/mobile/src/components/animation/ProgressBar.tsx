import { MotiView } from 'moti';
import { memo } from 'react';
import { View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

type ProgressBarProps = {
	value: number;
	total: number;
	pending?: boolean;
};

export const ProgressBar = memo((props: ProgressBarProps) => {
	const percentage = props.pending ? 0 : Math.round((props.value / props.total) * 100);

	return (
		<View style={tw`h-1 w-[94%] overflow-hidden rounded-full bg-app-button`}>
			<MotiView
				style={tw`h-full bg-accent`}
				animate={{ width: `${percentage}%` }}
				transition={{ type: 'timing' }}
			/>
		</View>
	);
});
