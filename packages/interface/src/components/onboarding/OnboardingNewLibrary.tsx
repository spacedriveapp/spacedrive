import Database from '@sd/assets/images/Database.png';
import { Button, Input } from '@sd/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router';

import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { useUnlockOnboardingScreen } from './OnboardingProgress';
import {
	OnboardingContainer,
	OnboardingDescription,
	OnboardingImg,
	OnboardingTitle
} from './OnboardingRoot';

export default function OnboardingNewLibrary() {
	const navigate = useNavigate();
	const [open, setOpen] = useState(false);

	useUnlockOnboardingScreen();

	return (
		<OnboardingContainer>
			<OnboardingImg src={Database} />
			<OnboardingTitle>Create a Library</OnboardingTitle>
			<OnboardingDescription>
				Libraries are a secure, on-device database. Your files remain where they are, the Library
				catalogs them and stores all Spacedrive related data.
			</OnboardingDescription>
			<Input
				//@ts-expect-error - size prop conflicts for some reason, despite being a valid variant
				size="md"
				autoFocus
				className="mt-6 w-[300px]"
				placeholder={'e.g. "James\' Library"'}
			/>
			<div className="space-x-2 mt-7">
				<Button onClick={() => navigate('/onboarding/privacy')} variant="accent" size="sm">
					New library
				</Button>
				<span className="px-2 text-xs font-bold text-ink-faint">OR</span>
				<CreateLibraryDialog open={open} setOpen={setOpen} onSubmit={() => navigate('/overview')}>
					<Button variant="outline" size="sm">
						Import library
					</Button>
				</CreateLibraryDialog>
			</div>
		</OnboardingContainer>
	);
}
