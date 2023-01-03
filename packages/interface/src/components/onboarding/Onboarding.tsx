import clsx from 'clsx';
import { useState } from 'react';
import { useNavigate } from 'react-router';

import { Button, Input } from '../../../../ui/src';
import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';

// TODO: This page requires styling for now it is just a placeholder.
export default function OnboardingPage() {
	const os = useOperatingSystem();
	const navigate = useNavigate();
	const [open, setOpen] = useState(false);

	return (
		<div
			className={clsx(
				'h-screen p-10 flex flex-col justify-center',
				os !== 'macOS' && 'bg-white dark:bg-black'
			)}
		>
			<h1 className="text-red-500">Welcome to Spacedrive</h1>
			<CreateLibraryDialog open={open} setOpen={setOpen} onSubmit={() => navigate('/overview')}>
				<Button variant="accent" size="sm">
					Create your library
				</Button>
			</CreateLibraryDialog>
		</div>
	);
}
