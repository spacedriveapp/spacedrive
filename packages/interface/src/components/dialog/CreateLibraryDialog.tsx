import { useBridgeMutation } from '@sd/client';
import { Input } from '@sd/ui';
import { Dialog } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { PropsWithChildren, useState } from 'react';

export default function CreateLibraryDialog({
	children,
	onSubmit,
	open,
	setOpen
}: PropsWithChildren<{ onSubmit?: () => void; open: boolean; setOpen: (state: boolean) => void }>) {
	const [newLibName, setNewLibName] = useState('');

	const queryClient = useQueryClient();
	const { mutate: createLibrary, isLoading: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: (library: any) => {
				setOpen(false);

				queryClient.setQueryData(['library.list'], (libraries: any) => [
					...(libraries || []),
					library
				]);

				if (onSubmit) onSubmit();
			},
			onError: (err: any) => {
				console.error(err);
			}
		}
	);

	return (
		<Dialog
			open={open}
			setOpen={setOpen}
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
				required
			/>
		</Dialog>
	);
}
