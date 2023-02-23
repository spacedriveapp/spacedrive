import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';

export default function NotFound() {
	const navigate = useNavigate();
	return (
		<div className="bg-app/80 w-full">
			<div
				role="alert"
				className="flex h-full w-full flex-col items-center justify-center rounded-lg p-4"
			>
				<p className="text-ink-faint m-3 text-sm font-semibold uppercase">Error: 404</p>
				<h1 className="text-4xl font-bold">There's nothing here.</h1>
				<p className="text-ink-dull mt-2 text-sm">
					Its likely that this page has not been built yet, if so we're on it!
				</p>
				<div className="flex flex-row space-x-2">
					<Button variant="outline" className="mt-4" onClick={() => navigate(-1)}>
						← Go Back
					</Button>
				</div>
			</div>
		</div>
	);
}
