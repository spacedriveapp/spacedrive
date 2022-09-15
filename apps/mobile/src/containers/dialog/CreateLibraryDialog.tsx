import React, { useState } from 'react';
import { useSnapshot } from 'valtio';
import Dialog from '~/components/layout/Dialog';
import { TextInput } from '~/components/primitive/Input';
import { useBridgeMutation } from '~/hooks/rspc';
import { libraryStore } from '~/stores/libraryStore';

type Props = {
	onSubmit?: () => void;
	disableBackdropClose?: boolean;
	children: React.ReactNode;
};

const CreateLibraryDialog = ({ children, onSubmit, disableBackdropClose }: Props) => {
	const [libName, setLibName] = useState('');
	const [createLibOpen, setCreateLibOpen] = useState(false);

	const { switchLibrary } = useSnapshot(libraryStore);

	const { mutate: createLibrary, isLoading: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: (data) => {
				// Reset form
				setLibName('');
				// Switch to the new library
				switchLibrary(data.uuid);

				if (onSubmit) onSubmit();
			},
			onSettled: () => {
				// Close create lib dialog
				setCreateLibOpen(false);
			}
		}
	);
	return (
		<Dialog
			isVisible={createLibOpen}
			setIsVisible={setCreateLibOpen}
			title="Create New Library"
			description="Choose a name for your new library, you can configure this and more settings from the library settings later on."
			ctaLabel="Create"
			ctaAction={() => createLibrary(libName)}
			loading={createLibLoading}
			ctaDisabled={libName.length === 0}
			trigger={children}
			disableBackdropClose={disableBackdropClose}
		>
			<TextInput
				value={libName}
				onChangeText={(text) => setLibName(text)}
				placeholder="My Cool Library"
			/>
		</Dialog>
	);
};

export default CreateLibraryDialog;
