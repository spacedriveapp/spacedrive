import { useState } from 'react';
import { useNavigate } from 'react-router';

import { Button } from '../../../../ui/src';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';

// TODO: This page requires styling for now it is just a placeholder.
export default function OnboardingPage() {
	const navigate = useNavigate();
	const [open, setOpen] = useState(false);

	return (
		<div className="h-screen p-10 flex flex-col justify-center bg-gradient-to-t from-accent to-purple-600">
			<h1 className="text-white font-bold text-center text-4xl mb-4">Welcome to Spacedrive</h1>
			<CreateLibraryDialog open={open} setOpen={setOpen} onSubmit={() => navigate('/overview')}>
				<Button variant="accent" size="md">
					Create your library
				</Button>
			</CreateLibraryDialog>
		</div>
	);
}
