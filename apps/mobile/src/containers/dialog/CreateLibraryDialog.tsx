import React, { useState } from 'react';
import Dialog from '~/components/layout/Dialog';
import { TextInput } from '~/components/primitive/Input';
import { useBridgeMutation } from '~/hooks/rspc';

type Props = {
	onSubmit?: () => void;
	children: React.ReactNode;
};

const CreateLibraryDialog = ({ children, onSubmit }: Props) => {
	const [libName, setLibName] = useState('');
	const [createLibOpen, setCreateLibOpen] = useState(false);

	const { mutate: createLibrary, isLoading: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: () => {
				// Reset form
				setLibName('');
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
