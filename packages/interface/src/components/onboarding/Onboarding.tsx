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
			<div className="text-white mt-2 mb-4">
				<p className="text-sm mb-1">
					The default keymanager details are below. This is only for development, and will be
					completely random once onboarding has completed. The secret key is just 16x zeroes encoded
					in hex.
				</p>
				<div className="flex space-x-2">
					<div className="relative flex">
						<p className="mr-2 text-sm mt-2">Password:</p>
						<Input value="password" className="flex-grow !py-0.5" disabled />
					</div>
					<div className="relative flex w-[375px]">
						<p className="mr-2 text-sm mt-2">Secret Key:</p>
						<Input
							value="30303030-30303030-30303030-30303030"
							className="flex-grow !py-0.5"
							disabled
						/>
					</div>
				</div>
			</div>

			<CreateLibraryDialog open={open} setOpen={setOpen} onSubmit={() => navigate('/overview')}>
				<Button variant="accent" size="sm">
					Create your library
				</Button>
			</CreateLibraryDialog>
		</div>
	);
}
