import { Button } from '@sd/ui';
import React from 'react';
import { useNavigate } from 'react-router';

export function NotFound() {
	const navigate = useNavigate();
	return (
		<div
			data-tauri-drag-region
			role="alert"
			className="flex flex-col items-center justify-center w-full h-full p-4 rounded-lg dark:text-white"
		>
			<p className="m-3 mt-20 text-sm font-semibold text-gray-500 uppercase">Error: 404</p>
			<h1 className="text-4xl font-bold">You chose nothingness.</h1>
			<div className="flex flex-row space-x-2">
				<Button variant="primary" className="mt-4" onClick={() => navigate(-1)}>
					Go Back
				</Button>
			</div>
		</div>
	);
}
