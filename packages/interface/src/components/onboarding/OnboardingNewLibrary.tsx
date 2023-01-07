import { Button } from '@sd/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router';

import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { useOnboardingScreenMounted } from './OnboardingProgress';

export default function OnboardingNewLibrary() {
	const navigate = useNavigate();
	const [open, setOpen] = useState(false);

	useOnboardingScreenMounted();

	return (
		<>
			<div className="flex flex-col items-center ">
				<h2 className="mb-2 text-3xl font-bold">Your Library</h2>
				<p className="max-w-xl text-center text-ink-dull">
					Libraries are where Spacedrive stores data, they do not contain files, just knowledge of
					the files, metadata and settings.
				</p>
				<div className="space-x-2 mt-7">
					<CreateLibraryDialog open={open} setOpen={setOpen} onSubmit={() => navigate('/overview')}>
						<Button variant="accent" size="sm">
							New library
						</Button>
					</CreateLibraryDialog>
					<Button variant="outline" size="sm">
						Import library
					</Button>
				</div>
			</div>
		</>
	);
}
