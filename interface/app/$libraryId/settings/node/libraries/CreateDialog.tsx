import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { LibraryConfigWrapped, useBridgeMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, UseDialogProps, forms, useDialog } from '@sd/ui';

const { Input, z, useZodForm } = forms;

const schema = z.object({
	name: z.string().min(1)
});
export default (props: UseDialogProps) => {
	const dialog = useDialog(props);
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const submitPlausibleEvent = usePlausibleEvent();

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(
				['library.list'],
				(libraries: LibraryConfigWrapped[] | undefined) => [...(libraries || []), library]
			);

			submitPlausibleEvent({
				event: {
					type: 'libraryCreate'
				}
			});

			navigate(`/${library.uuid}/overview`);
		},
		onError: (err) => console.log(err)
	});

	const form = useZodForm({
		schema: schema
	});

	const onSubmit = form.handleSubmit(async (data) => {
		await createLibrary.mutateAsync({
			name: data.name
		});
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			submitDisabled={!form.formState.isValid}
			title="Create New Library"
			description="Libraries are a secure, on-device database. Your files remain where they are, the Library catalogs them and stores all Spacedrive related data."
			ctaLabel={form.formState.isSubmitting ? 'Creating library...' : 'Create library'}
		>
			<div className="mt-5 space-y-4">
				<Input
					{...form.register('name')}
					label="Library name"
					placeholder={'e.g. "James\' Library"'}
					size="md"
				/>
			</div>
		</Dialog>
	);
};
