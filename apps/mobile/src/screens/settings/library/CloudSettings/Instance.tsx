import { Text, View } from 'react-native';
import { CloudInstance, HardwareModel } from '@sd/client';
import { Icon } from '~/components/icons/Icon';
import { hardwareModelToIcon } from '~/components/overview/Devices';
import { tw } from '~/lib/tailwind';

import { InfoBox } from './CloudSettings';

interface Props {
	data: CloudInstance;
}

const Instance = ({ data }: Props) => {
	return (
		<InfoBox style={tw`w-full gap-2`}>
			<View>
				<View style={tw`mx-auto my-2`}>
					<Icon
						name={
							hardwareModelToIcon(data.metadata.device_model as HardwareModel) as any
						}
						size={60}
					/>
				</View>
				<Text
					numberOfLines={1}
					style={tw`mb-3 px-1 text-center text-sm font-medium font-semibold text-ink`}
				>
					{data.metadata.name}
				</Text>
				<InfoBox>
					<View style={tw`flex-row items-center gap-1`}>
						<Text style={tw`text-sm font-medium text-ink`}>Id:</Text>
						<Text numberOfLines={1} style={tw`max-w-[250px] text-ink-dull`}>
							{data.id}
						</Text>
					</View>
				</InfoBox>
			</View>
			<View>
				<InfoBox>
					<View style={tw`flex-row items-center gap-1`}>
						<Text style={tw`text-sm font-medium text-ink`}>UUID:</Text>
						<Text numberOfLines={1} style={tw`max-w-[85%] text-ink-dull`}>
							{data.uuid}
						</Text>
					</View>
				</InfoBox>
			</View>
			<View>
				<InfoBox>
					<View style={tw`flex-row items-center gap-1`}>
						<Text style={tw`text-sm font-medium text-ink`}>Public key:</Text>
						<Text numberOfLines={1} style={tw`max-w-3/4 text-ink-dull`}>
							{data.identity}
						</Text>
					</View>
				</InfoBox>
			</View>
		</InfoBox>
	);
};

export default Instance;
