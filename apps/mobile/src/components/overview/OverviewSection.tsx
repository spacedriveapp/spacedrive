import { PropsWithChildren } from 'react';
import { Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';

interface Props extends PropsWithChildren {
	title: string;
	count?: number;
}

const OverviewSection = ({ title, count, children }: Props) => {
	return (
		<View>
			{/* The view wrapper is needed to prevent gap spacing from screen container */}
			<View style={tw`flex-row items-center gap-3 px-6 pb-3`}>
				<Text style={tw`text-lg font-bold text-white`}>{title}</Text>
				<View
					style={tw`flex h-[24px] w-[24px] items-center justify-center rounded-full border border-app-button/40 px-1`}
				>
					<Text style={tw`text-xs text-ink`}>{count}</Text>
				</View>
			</View>
			{children}
		</View>
	);
};

export default OverviewSection;
