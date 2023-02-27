import { useState } from 'react';
import { Switch } from '@sd/ui';
import { InputContainer } from '~/components/primitive/InputContainer';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';

export default function PrivacySettings() {
	const [shareUsageData, setShareUsageData] = useState(true);
	const [blurEffects, setBlurEffects] = useState(true);
	return (
		<SettingsContainer>
			<SettingsHeader title="Privacy" description="" />
			<InputContainer
				mini
				title="Share Usage Data"
				description="Share anonymous usage data to help us improve the app."
			>
				<Switch checked={shareUsageData} onCheckedChange={setShareUsageData} className="m-2 ml-4" />
			</InputContainer>
		</SettingsContainer>
	);
}
