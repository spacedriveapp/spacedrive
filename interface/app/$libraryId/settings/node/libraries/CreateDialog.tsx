import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { LibraryConfigWrapped, useBridgeMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, UseDialogProps, forms, useDialog } from '@sd/ui';

const { Input, z, useZodForm } = forms;

const schema = z.object({
	name: z
		.string()
		.min(1)
		.refine((v) => !v.startsWith(' ') && !v.endsWith(' '), {
			message: "Name can't start or end with a space",
			path: ['name']
		})
});

export default (props: UseDialogProps) => {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const submitPlausibleEvent = usePlausibleEvent();

	const createLibrary = useBridgeMutation('library.create');

	const form = useZodForm({ schema });

	const onSubmit = form.handleSubmit(async (data) => {
		const library = await createLibrary.mutateAsync({ name: data.name });

		queryClient.setQueryData<LibraryConfigWrapped[]>(['library.list'], (libraries) => [
			...(libraries || []),
			library
		]);

		submitPlausibleEvent({
			event: { type: 'libraryCreate' }
		});

		navigate(`/${library.uuid}/overview`);
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={useDialog(props)}
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
