import { useForm } from 'react-hook-form';
import { useBridgeMutation, useLibraryContext } from '@sd/client';
import { Button, Input, Switch, dialogManager } from '@sd/ui';
import DeleteLibraryDialog from '~/components/dialog/DeleteLibraryDialog';
import { InputContainer } from '~/components/primitive/InputContainer';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';
import { useDebouncedFormWatch } from '~/hooks/useDebouncedForm';

export default function LibraryGeneralSettings() {
	const { library } = useLibraryContext();
	const editLibrary = useBridgeMutation('library.edit');

	const form = useForm({
		defaultValues: { id: library!.uuid, ...library?.config }
	});

	useDebouncedFormWatch(form, (value) =>
		editLibrary.mutate({
			id: library.uuid,
			name: value.name ?? null,
			description: value.description ?? null
		})
	);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Library Settings"
				description="General settings related to the currently active library."
			/>
			<div className="flex flex-row space-x-5 pb-3">
				<div className="flex grow flex-col">
					<span className="mb-1 text-sm font-medium">Name</span>
					<Input
						size="md"
						{...form.register('name', { required: true })}
						defaultValue="My Default Library"
					/>
				</div>
				<div className="flex grow flex-col">
					<span className="mb-1 text-sm font-medium">Description</span>
					<Input size="md" {...form.register('description')} placeholder="" />
				</div>
			</div>

			<InputContainer
				mini
				title="Encrypt Library"
				description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves."
			>
				<div className="ml-3 flex items-center">
					<Switch checked={false} />
				</div>
			</InputContainer>
			<InputContainer mini title="Export Library" description="Export this library to a file.">
				<div className="mt-2">
					<Button size="sm" variant="gray">
						Export
					</Button>
				</div>
			</InputContainer>
			<InputContainer
				mini
				title="Delete Library"
				description="This is permanent, your files will not be deleted, only the Spacedrive library."
			>
				<div className="mt-2">
					<Button
						size="sm"
						variant="colored"
						className="border-red-500 bg-red-500"
						onClick={() => {
							dialogManager.create((dp) => (
								<DeleteLibraryDialog {...dp} libraryUuid={library.uuid} />
							));
						}}
					>
						Delete
					</Button>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
