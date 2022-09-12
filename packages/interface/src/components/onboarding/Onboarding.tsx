import { useNavigate } from 'react-router';

import { Button } from '../../../../ui/src';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';

export default function OnboardingPage() {
	const navigate = useNavigate();
	return (
		<div className="p-10 flex flex-col justify-center">
			<h1 className="text-red-500">Welcome to Spacedrive</h1>

			<CreateLibraryDialog onSubmit={() => navigate('overview')}>
				<Button variant="primary" size="sm">
					Create your library
				</Button>
			</CreateLibraryDialog>
		</div>
	);
}
