import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Button, Form, InputField } from '@sd/ui';
import { Icon } from '~/components';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';
import { useOnboardingContext } from './context';

export default function OnboardingNewLibrary() {
	const navigate = useNavigate();
	const form = useOnboardingContext().forms.useForm('new-library');

	const [importMode, setImportMode] = useState(false);

	const handleImport = () => {
		// TODO
	};

	return (
		<Form
			form={form}
			onSubmit={form.handleSubmit(() => {
				navigate('../locations', { replace: true });
			})}
		>
			<OnboardingContainer>
				<Icon name="Database" size={80} className="mb-2" />
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
						<InputField
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
								size="sm"
								disabled={!form.formState.isValid}
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
