import { Database } from '@sd/assets/icons';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { getOnboardingStore, useOnboardingStore } from '@sd/client';
import { Button } from '@sd/ui';
import { Form, Input, useZodForm, z } from '@sd/ui/src/forms';
import {
	OnboardingContainer,
	OnboardingDescription,
	OnboardingImg,
	OnboardingTitle
} from './Layout';
import { useUnlockOnboardingScreen } from './Progress';

const schema = z.object({
	// the regex here validates that the entire string isn't purely whitespace
	name: z.string().min(1, 'Name is required').regex(/[\S]/g).trim()
});

export default function OnboardingNewLibrary() {
	const navigate = useNavigate();
	const [importMode, setImportMode] = useState(false);

	const obStore = useOnboardingStore();

	const form = useZodForm({
		schema,
		defaultValues: {
			name: obStore.newLibraryName
		}
	});

	useUnlockOnboardingScreen();

	const onSubmit = form.handleSubmit(async (data) => {
		getOnboardingStore().newLibraryName = data.name;
		navigate('/onboarding/privacy', { replace: true });
	});

	const handleImport = () => {
		// TODO
	};

	return (
		<Form form={form} onSubmit={onSubmit}>
			<OnboardingContainer>
				<OnboardingImg src={Database} />
				<OnboardingTitle>Create a Library</OnboardingTitle>
				<OnboardingDescription>
					Libraries are a secure, on-device database. Your files remain where they are,
					the Library catalogs them and stores all Spacedrive related data.
				</OnboardingDescription>

				{importMode ? (
					<div className="mt-7 space-x-2">
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
							size="lg"
							autoFocus
							className="mt-6 w-[300px]"
							placeholder={'e.g. "James\' Library"'}
						/>
						<div className="flex grow" />
						<div className="mt-7 space-x-2">
							<Button
								type="submit"
								variant="accent"
								disabled={!form.formState.isValid}
								size="sm"
							>
								New library
							</Button>
							{/* <span className="px-2 text-xs font-bold text-ink-faint">OR</span>
							<Button onClick={() => setImportMode(true)} variant="outline" size="sm">
								Import library
							</Button> */}
						</div>
					</>
				)}
			</OnboardingContainer>
		</Form>
	);
}
