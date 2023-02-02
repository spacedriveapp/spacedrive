import { useNavigate } from 'react-router';
import { Button, dialogManager } from '@sd/ui';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';

// TODO: This page requires styling for now it is just a placeholder.
export default function OnboardingPage() {
	const navigate = useNavigate();

	return (
		<div className="from-accent flex h-screen flex-col justify-center bg-gradient-to-t to-purple-600 p-10">
			<h1 className="mb-4 text-center text-4xl font-bold text-white">Welcome to Spacedrive</h1>
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
