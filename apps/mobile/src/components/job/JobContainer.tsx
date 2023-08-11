import { Icon } from 'phosphor-react-native';
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
				'border-b border-app-line/50 px-4 py-3',
				isChild && 'border-b-0 bg-app-darkBox p-2 pl-10'
			)}
		>
			{typeof Icon === 'number' ? (
				<Image source={Icon} style={tw`h-8 w-8`} />
			) : (
				Icon && <Icon weight="fill" color="white" style={tw``} />
			)}
			<MetaContainer>
				{textItems?.map((item, index) => {
					// filter out undefined text so we don't render empty TextItems
					const filteredItems = item.filter((i) => i?.text);
					const popoverText = filteredItems.map((i) => i?.text).join(' • ');
					// TODO:
					return (
						<Text key={index} style={tw`mr-8 mt-[2px] pl-1.5 text-ink-faint`}>
							{filteredItems.map((item, index) => {
								const Icon = item?.icon;
								return (
									<>
										{Icon && (
											<Icon
												weight="fill"
												className="-mt-0.5 ml-[5px] mr-1 inline"
											/>
										)}
										<Text key={index} style={tw`truncate`}>
											{item?.text}
										</Text>
										{index < filteredItems.length - 1 && <Text> • </Text>}
									</>
								);
							})}
						</Text>
					);
				})}
				<View style={tw`mt-1`}>{children}</View>
			</MetaContainer>
		</View>
	);
}
