import { CloudLibrary, HardwareModel, useLibraryContext } from '@sd/client';
import { useMemo } from 'react';
import { Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import { Divider } from '~/components/primitive/Divider';
import { tw } from '~/lib/tailwind';

import { Icon } from '~/components/icons/Icon';
import { hardwareModelToIcon } from '~/components/overview/Devices';
import { InfoBox } from './CloudSettings';

interface ThisInstanceProps {
	cloudLibrary?: CloudLibrary;
}

const ThisInstance = ({ cloudLibrary }: ThisInstanceProps) => {
	const { library } = useLibraryContext();
	const thisInstance = useMemo(
		() => cloudLibrary?.instances.find((instance) => instance.uuid === library.instance_id),
		[cloudLibrary, library.instance_id]
	);

	if (!thisInstance) return null;

	return (
		<Card style={tw`w-full gap-2`}>
			<View>
				<Text style={tw`mb-1 font-semibold text-ink`}>This Instance</Text>
				<Divider />
			</View>
			<View style={tw`mx-auto my-2 items-center`}>
			<Icon
				name={
				hardwareModelToIcon(
				thisInstance.metadata.device_model as HardwareModel) as any}
				size={60}
				/>
			<Text numberOfLines={1} style={tw`px-1 font-semibold text-ink`}>{thisInstance.metadata.name}</Text>
			</View>
			<View>
				<InfoBox>
				<View style={tw`flex-row items-center gap-1`}>
				<Text style={tw`text-sm font-medium text-ink`}>Id:</Text>
					<Text style={tw`max-w-[250px] text-ink-dull`}>{thisInstance.id}</Text>
				</View>
				</InfoBox>
			</View>
			<View>
				<InfoBox>
				<View style={tw`flex-row items-center gap-1`}>
				<Text style={tw`text-sm font-medium text-ink`}>UUID:</Text>
					<Text numberOfLines={1} style={tw`max-w-[85%] text-ink-dull`}>{thisInstance.uuid}</Text>
				</View>
				</InfoBox>
			</View>
			<View>
				<InfoBox>
				<View style={tw`flex-row items-center gap-1`}>
				<Text style={tw`text-sm font-medium text-ink`}>Publc Key:</Text>
					<Text numberOfLines={1} style={tw`max-w-3/4 text-ink-dull`}>
						{thisInstance.identity}
					</Text>
				</View>
				</InfoBox>
			</View>
		</Card>
	);
};

export default ThisInstance;
