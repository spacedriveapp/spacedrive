import { Switch } from '@sd/ui';
import { useState } from 'react';

import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function PrivacySettings() {
	const [uiAnimations, setUiAnimations] = useState(true);
	const [blurEffects, setBlurEffects] = useState(true);
	return (
		<SettingsContainer>
			<SettingsHeader title="Permissions" description="" />
			<InputContainer
				mini
				title="UI Animations"
				description="Dialogs and other UI elements will animate when opening and closing."
			>
				<Switch checked={uiAnimations} onCheckedChange={setUiAnimations} className="m-2 ml-4" />
			</InputContainer>
			<InputContainer
				mini
				title="Blur Effects"
				description="Some components will have a blur effect applied to them."
			>
				<Switch checked={blurEffects} onCheckedChange={setBlurEffects} className="m-2 ml-4" />
			</InputContainer>
		</SettingsContainer>
	);
}
