import { useBridgeMutation } from '@sd/client';
import { Input } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { PropsWithChildren, useState } from 'react';

import Dialog from '../layout/Dialog';

export default function CreateLibraryDialog({ children }: PropsWithChildren) {
	const [openCreateModal, setOpenCreateModal] = useState(false);
	const [newLibName, setNewLibName] = useState('');

	const queryClient = useQueryClient();
	const { mutate: createLibrary, isLoading: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: () => {
				queryClient.invalidateQueries(['library.get']); // TODO: Change to `library.list`
				setOpenCreateModal(false);
			},
			onError: (err) => {
				console.error(err);
			}
		}
	);

	return (
		<Dialog
			open={openCreateModal}
			onOpenChange={setOpenCreateModal}
			title="Create New Library"
			description="Choose a name for your new library, you can configure this and more settings from the library settings later on."
			ctaAction={() => createLibrary(newLibName)}
			loading={createLibLoading}
			submitDisabled={!newLibName}
			ctaLabel="Create"
			trigger={children}
		>
			<Input
				className="flex-grow w-full mt-3"
				value={newLibName}
				placeholder="My Cool Library"
				onChange={(e) => setNewLibName(e.target.value)}
			/>
		</Dialog>
	);
}
