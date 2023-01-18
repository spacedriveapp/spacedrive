import Database from '@sd/assets/images/Database.png';
import { getOnboardingStore, useOnboardingStore } from '@sd/client';
import { Button } from '@sd/ui';
import { forms } from '@sd/ui';
import { BaseSyntheticEvent, useEffect, useState } from 'react';
import { useNavigate } from 'react-router';

import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { useUnlockOnboardingScreen } from './OnboardingProgress';
import {
	OnboardingContainer,
	OnboardingDescription,
	OnboardingImg,
	OnboardingTitle
} from './OnboardingRoot';

const { Input, z, useZodForm, Form } = forms;

const schema = z.object({
	name: z.string()
});

export default function OnboardingNewLibrary() {
	const navigate = useNavigate();
	const [importMode, setImportMode] = useState(false);

	const ob_store = useOnboardingStore();

	const form = useZodForm({
		schema,
		defaultValues: {
			// not sure why this needs "as string"? valtio...
			name: (ob_store.newLibraryName as string) || ''
		}
	});

	useUnlockOnboardingScreen();

	const _onSubmit = form.handleSubmit(async (data) => {
		getOnboardingStore().newLibraryName = data.name;
		navigate('/onboarding/master-password');
		return;
	});

	const handleImport = () => {
		return;
	};

	return (
		<Form form={form} onSubmit={_onSubmit}>
			<OnboardingContainer>
				<OnboardingImg src={Database} />
				<OnboardingTitle>Create a Library</OnboardingTitle>
				<OnboardingDescription>
					Libraries are a secure, on-device database. Your files remain where they are, the Library
					catalogs them and stores all Spacedrive related data.
				</OnboardingDescription>

				{importMode ? (
					<div className="space-x-2 mt-7">
						<Button onClick={handleImport} variant="accent" size="sm">
							Import
						</Button>
						<span className="px-2 text-xs font-bold text-ink-faint">OR</span>
						<Button onClick={() => setImportMode(false)} variant="outline" size="sm">
							Create new library
						</Button>
					</div>
				) : (
					<>
						<Input
							{...form.register('name')}
							//@ts-expect-error - size prop conflicts for some reason, despite being a valid variant
							size="md"
							autoFocus
							className="mt-6 w-[300px]"
							placeholder={'e.g. "James\' Library"'}
						/>
						<div className="flex flex-grow" />
						<div className="space-x-2 mt-7">
							<Button type="submit" variant="accent" size="sm">
								New library
							</Button>
							<span className="px-2 text-xs font-bold text-ink-faint">OR</span>
							<Button onClick={() => setImportMode(true)} variant="outline" size="sm">
								Import library
							</Button>
						</div>
					</>
				)}
			</OnboardingContainer>
		</Form>
	);
}
