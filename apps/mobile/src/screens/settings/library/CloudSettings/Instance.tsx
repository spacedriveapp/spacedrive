import { Text, View } from 'react-native';
import { CloudInstance } from '@sd/client';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { tw, twStyle } from '~/lib/tailwind';

import { InfoBox } from './CloudSettings';

interface Props {
	data: CloudInstance;
	length: number;
}

const Instance = ({ data, length }: Props) => {
	return (
		<InfoBox style={twStyle(length > 1 ? 'w-[49%]' : 'w-full', 'gap-4')}>
			<View>
				<SettingsTitle style={tw`mb-2`}>Id</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>
						{data.id}
					</Text>
				</InfoBox>
			</View>
			<View>
				<SettingsTitle style={tw`mb-2`}>UUID</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>
						{data.uuid}
					</Text>
				</InfoBox>
			</View>
			<View>
				<SettingsTitle style={tw`mb-2`}>Public Key</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>
						{data.identity}
					</Text>
				</InfoBox>
			</View>
		</InfoBox>
	);
};

export default Instance;
