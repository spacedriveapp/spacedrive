import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { LibraryConfigWrapped, useBridgeMutation, usePlausibleEvent } from '@sd/client';
import { Dialog, InputField, UseDialogProps, useDialog, useZodForm, z } from '@sd/ui';

const schema = z.object({
	name: z.string().min(1)
});

export default (props: UseDialogProps) => {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const submitPlausibleEvent = usePlausibleEvent();
	const form = useZodForm({ schema });

	const createLibrary = useBridgeMutation('library.create');

	const onSubmit = form.handleSubmit(async (data) => {
		try {
			const library = await createLibrary.mutateAsync({ name: data.name });

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
		} catch (e) {
			console.error(e);
		}
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
				<InputField
					{...form.register('name')}
					label="Library name"
					placeholder={'e.g. "James\' Library"'}
					size="md"
				/>
			</div>
		</Dialog>
	);
};
