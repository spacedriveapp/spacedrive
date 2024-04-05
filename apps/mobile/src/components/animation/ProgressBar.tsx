import { MotiView } from 'moti';
import { memo } from 'react';
import { View } from 'react-native';
import { tw } from '~/lib/tailwind';

type ProgressBarProps = {
	value: number;
	total: number;
	pending?: boolean;
};

export const ProgressBar = memo((props: ProgressBarProps) => {
	const percentage = props.pending ? 0 : Math.round((props.value / props.total) * 100);

	if (props.pending) {
		// Show indeterminate progress bar
		return (
			<View style={tw`h-1 overflow-hidden rounded-full bg-app-button`}>
				<MotiView
					style={tw`h-full w-1/2 bg-accent`}
					from={{ left: '-50%' }}
					animate={{ left: '100%' }}
					transition={{ type: 'timing', duration: 1500, loop: true }}
				/>
			</View>
		);
	}

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
