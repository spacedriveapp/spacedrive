import { Button, dialogManager } from '@sd/ui';
import { useNavigate } from 'react-router';

import CreateLibraryDialog from '../dialog/CreateLibraryDialog';

// TODO: This page requires styling for now it is just a placeholder.
export default function OnboardingPage() {
	const navigate = useNavigate();

	return (
		<div className="h-screen p-10 flex flex-col justify-center bg-gradient-to-t from-accent to-purple-600">
			<h1 className="text-white font-bold text-center text-4xl mb-4">Welcome to Spacedrive</h1>
			<Button
				variant="accent"
				size="md"
				onClick={() => {
					dialogManager.create((dp) => <CreateLibraryDialog {...dp} />, {
						onSubmit: () => navigate('/overview')
					});
				}}
			>
				Create your library
			</Button>
		</div>
	);
}
