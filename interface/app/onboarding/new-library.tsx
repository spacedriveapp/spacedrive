import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Button, Form, InputField } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale, useOperatingSystem } from '~/hooks';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './components';
import { useOnboardingContext } from './context';

export default function OnboardingNewLibrary() {
	const { t } = useLocale();

	const navigate = useNavigate();
	const os = useOperatingSystem();
	const form = useOnboardingContext().forms.useForm('new-library');

	const [importMode, setImportMode] = useState(false);

	const handleImport = () => {
		// TODO
	};

	return (
		<Form
			form={form}
			onSubmit={form.handleSubmit(() => {
				navigate(`../${os === 'macOS' ? 'full-disk' : 'locations'}`, { replace: true });
			})}
		>
			<OnboardingContainer>
				<Icon name="Database" size={80} />
				<OnboardingTitle>{t('create_library')}</OnboardingTitle>
				<OnboardingDescription>{t('create_library_description')}</OnboardingDescription>

				{importMode ? (
					<div className="mt-7 space-x-2">
						<Button onClick={handleImport} variant="accent" size="sm">
							{t('import')}
						</Button>
						<span className="px-2 text-xs font-bold text-ink-faint">{t('or')}</span>
						<Button onClick={() => setImportMode(false)} variant="outline" size="sm">
							{t('create_new_library')}
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
								{t('new_library')}
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
