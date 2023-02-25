import { Switch } from '@sd/ui';
import { useNodeStore } from '~/components/device/Stores';
import { InputContainer } from '~/components/primitive/InputContainer';
import { Header } from '../Layout';

export default function ExperimentalSettings() {
	const { isExperimental, setIsExperimental } = useNodeStore();

	return (
		<>
			{/* <Button size="sm">Add Location</Button> */}
			<Header title="Experimental" description="Experimental features within Spacedrive." />
			<InputContainer
				mini
				title="Debug Menu"
				description="Shows data about Spacedrive such as Jobs, Job History and Client State."
			>
				<div className="flex h-full items-center pl-10">
					<Switch
						checked={isExperimental}
						size="sm"
						onChange={() => setIsExperimental(!isExperimental)}
					/>
				</div>
			</InputContainer>
		</>
	);
}
