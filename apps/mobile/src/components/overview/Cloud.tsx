import { Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';

import { Button } from '../primitive/Button';
import NewCard from './NewCard';
import OverviewSection from './OverviewSection';

const Cloud = () => {
	return (
		<OverviewSection title="Cloud Drives" count={0}>
			<View style={tw`px-7`}>
				<NewCard
					icons={['DriveAmazonS3', 'DriveDropbox', 'DriveGoogleDrive', 'DriveOneDrive']}
					text="Connect your cloud accounts to Spacedrive."
					button={() => (
						<Button variant="transparent">
							<Text style={tw`font-bold text-ink-dull`}>Coming soon</Text>
						</Button>
					)}
				/>
			</View>
		</OverviewSection>
	);
};

export default Cloud;
