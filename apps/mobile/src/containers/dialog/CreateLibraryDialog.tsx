import { queryClient, useBridgeMutation, useCurrentLibrary } from '@sd/client';
import { useState } from 'react';
import Dialog from '~/components/layout/Dialog';
import { Input } from '~/components/primitive/Input';

type Props = {
	onSubmit?: () => void;
	disableBackdropClose?: boolean;
	children: React.ReactNode;
};

const CreateLibraryDialog = ({ children, onSubmit, disableBackdropClose }: Props) => {
	const [libName, setLibName] = useState('');
	const [createLibOpen, setCreateLibOpen] = useState(false);

	const { switchLibrary } = useCurrentLibrary();

	const { mutate: createLibrary, isLoading: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: (lib) => {
				// Reset form
				setLibName('');

				queryClient.setQueryData(['library.list'], (libraries: any) => [...(libraries || []), lib]);

				// Switch to the new library
				switchLibrary(lib.uuid);

				onSubmit?.();
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
			onClose={() => setLibName('')} // Reset form onClose
		>
			<Input
				value={libName}
				onChangeText={(text) => setLibName(text)}
				placeholder="My Cool Library"
			/>
		</Dialog>
	);
};

export default CreateLibraryDialog;
