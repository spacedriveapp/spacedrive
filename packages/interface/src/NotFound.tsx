import { useNavigate } from 'react-router';
import { Button } from '@sd/ui';

export default function NotFound() {
	const navigate = useNavigate();
	return (
		<div className="w-full bg-app/80">
			<div
				role="alert"
				className="flex flex-col items-center justify-center w-full h-full p-4 rounded-lg"
			>
				<p className="m-3 text-sm font-semibold uppercase text-ink-faint">Error: 404</p>
				<h1 className="text-4xl font-bold">There's nothing here.</h1>
				<p className="mt-2 text-sm text-ink-dull">
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
