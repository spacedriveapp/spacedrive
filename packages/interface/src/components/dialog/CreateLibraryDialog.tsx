import { queryClient, useBridgeMutation } from '@sd/client';
import { Input } from '@sd/ui';
import React, { useState } from 'react';

import Dialog from '../layout/Dialog';

interface Props {
	children: React.ReactNode;
}

export default function CreateLibraryDialog(props: Props) {
	const [openCreateModal, setOpenCreateModal] = useState(false);
	const [newLibName, setNewLibName] = useState('');

	const { mutate: createLibrary, isLoading: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: () => {
				setOpenCreateModal(false);
				queryClient.invalidateQueries(['library.list']);
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
			trigger={props.children}
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
