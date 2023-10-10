import { Icon } from 'phosphor-react-native';
import { Fragment } from 'react';
import { Image, Text, View, ViewStyle } from 'react-native';
import { TextItems } from '@sd/client';
import { styled, tw, twStyle } from '~/lib/tailwind';

type JobContainerProps = {
	name: string;
	icon?: string | Icon;
	// Array of arrays of TextItems, where each array of TextItems is a truncated line of text.
	textItems?: TextItems;
	isChild?: boolean;
	children: React.ReactNode;
	containerStyle?: ViewStyle;
};

const MetaContainer = styled(View, 'flex w-full overflow-hidden flex-col');

// Job container consolidates the common layout of a job item, used for regular jobs (Job.tsx) and grouped jobs (JobGroup.tsx).
export default function JobContainer(props: JobContainerProps) {
	const { name, icon: Icon, textItems, isChild, children, ...restProps } = props;

	return (
		<View
			style={twStyle(
				'flex flex-row justify-center',
				'border-b border-app-line/50 px-8 py-4',
				isChild && 'my-1.5 border-b-0 p-2 pl-12',
				restProps.containerStyle
			)}
		>
			{typeof Icon === 'number' ? (
				<Image source={Icon} style={tw`ml-4 mr-1 h-8 w-8`} />
			) : (
				Icon && (
					<View
						style={tw`mr-1 h-7 w-7 items-center justify-center rounded-full bg-app-button`}
					>
						<Icon weight="fill" color="white" size={18} />
					</View>
				)
			)}
			<MetaContainer>
				<Text style={tw`pl-1.5 text-sm font-medium text-white`} numberOfLines={1}>
					{name}
				</Text>
				{textItems?.map((item, index) => {
					// filter out undefined text so we don't render empty TextItems
					const filteredItems = item.filter((i) => i?.text);
					return (
						<Text
							key={index}
							style={tw`mt-[2px] pl-1.5 text-[13px] text-ink-faint`}
							numberOfLines={1}
						>
							{filteredItems.map((item, index) => {
								const Icon = item?.icon;
								return (
									<Fragment key={index}>
										{Icon && (
											<Icon
												weight="fill"
												// TODO: this might be ugly
												style={tw`-mt-0.5 ml-[5px] inline`}
											/>
										)}
										<Text key={index}>{item?.text}</Text>
										{index < filteredItems.length - 1 && <Text> • </Text>}
									</Fragment>
								);
							})}
						</Text>
					);
				})}
				{children && <View style={tw`mt-1`}>{children}</View>}
			</MetaContainer>
		</View>
	);
}
