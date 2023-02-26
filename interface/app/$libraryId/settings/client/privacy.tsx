import { useState } from 'react';
import { Switch } from '@sd/ui';
import { Heading } from '../Layout';
import Setting from '../Setting';

export default function PrivacySettings() {
	const [shareUsageData, setShareUsageData] = useState(true);
	const [blurEffects, setBlurEffects] = useState(true);

	return (
		<>
			<Heading title="Privacy" description="" />
			<Setting
				mini
				title="Share Usage Data"
				description="Share anonymous usage data to help us improve the app."
			>
				<Switch checked={shareUsageData} onCheckedChange={setShareUsageData} className="m-2 ml-4" />
			</Setting>
		</>
	);
}
