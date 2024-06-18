import { useMemo } from 'react';
import { Text, View } from 'react-native';
import { CloudLibrary, useLibraryContext } from '@sd/client';
import Card from '~/components/layout/Card';
import { Divider } from '~/components/primitive/Divider';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { tw } from '~/lib/tailwind';

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
		<Card style={tw`w-full gap-4`}>
			<View>
				<Text style={tw`mb-1 font-semibold text-ink`}>This Instance</Text>
				<Divider />
			</View>
			<View>
				<SettingsTitle style={tw`mb-2 text-ink`}>Id</SettingsTitle>
				<InfoBox>
					<Text style={tw`text-ink-dull`}>{thisInstance.id}</Text>
				</InfoBox>
			</View>
			<View>
				<SettingsTitle style={tw`mb-2`}>UUID</SettingsTitle>
				<InfoBox>
					<Text style={tw`text-ink-dull`}>{thisInstance.uuid}</Text>
				</InfoBox>
			</View>
			<View>
				<SettingsTitle style={tw`mb-2`}>Public Key</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>
						{thisInstance.identity}
					</Text>
				</InfoBox>
			</View>
		</Card>
	);
};

export default ThisInstance;
