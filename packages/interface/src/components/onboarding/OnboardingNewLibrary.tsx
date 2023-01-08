import { Button, Input } from '@sd/ui';
import { Books } from 'phosphor-react';
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
				<Books className="w-16 h-16 mb-2" />
				<h2 className="mb-2 text-3xl font-bold">Create a Library</h2>
				<p className="max-w-xl text-center text-ink-dull">
					Libraries are where Spacedrive stores metadata, they do not contain files, just knowledge
					of the files.
				</p>
				<Input autoFocus className="mt-6 w-[300px]" placeholder="Library Name" />
				<div className="space-x-2 mt-7">
					<CreateLibraryDialog open={open} setOpen={setOpen} onSubmit={() => navigate('/overview')}>
						<Button variant="accent" size="sm">
							New library
						</Button>
					</CreateLibraryDialog>
					<span className="px-2 text-xs font-bold text-ink-faint">OR</span>
					<Button variant="outline" size="sm">
						Import library
					</Button>
				</div>
			</div>
		</>
	);
}
