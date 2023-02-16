import { Switch } from '@sd/ui';
import { useNodeStore } from '~/components/device/Stores';
import { InputContainer } from '~/components/primitive/InputContainer';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';

export default function ExperimentalSettings() {
	const { isExperimental, setIsExperimental } = useNodeStore();

	return (
		<SettingsContainer>
			{/* <Button size="sm">Add Location</Button> */}
			<SettingsHeader title="Experimental" description="Experimental features within Spacedrive." />
			<InputContainer
				mini
				title="Debug Menu"
				description="Shows data about Spacedrive such as Jobs, Job History and Client State."
			>
				<div className="flex h-full items-center pl-10">
					<Switch
						checked={isExperimental}
						size="sm"
						onChange={(newValue) => {
							setIsExperimental(!isExperimental);
						}}
					/>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
