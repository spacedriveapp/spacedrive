import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';

export default function NotFound() {
	const navigate = useNavigate();
	return (
		<div
			data-tauri-drag-region
			role="alert"
			className="flex h-full w-full flex-col items-center justify-center rounded-lg p-4"
		>
			<p className="text-ink-faint m-3 text-sm font-semibold uppercase">Error: 404</p>
			<h1 className="text-4xl font-bold">You chose nothingness.</h1>
			<div className="flex flex-row space-x-2">
				<Button variant="accent" className="mt-4" onClick={() => navigate(-1)}>
					Go Back
				</Button>
			</div>
		</div>
	);
}
