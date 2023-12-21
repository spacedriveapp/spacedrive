import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import {
	insertLibrary,
	useBridgeMutation,
	useNormalisedCache,
	usePlausibleEvent,
	useZodForm
} from '@sd/client';
import { Dialog, InputField, useDialog, UseDialogProps, z } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

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
	const platform = usePlatform();

	const createLibrary = useBridgeMutation('library.create');

	const form = useZodForm({ schema });
	const cache = useNormalisedCache();

	const onSubmit = form.handleSubmit(async (data) => {
		try {
			const libraryRaw = await createLibrary.mutateAsync({
				name: data.name,
				default_locations: null
			});
			cache.withNodes(libraryRaw.nodes);
			const library = cache.withCache(libraryRaw.item);

			insertLibrary(queryClient, library);

			submitPlausibleEvent({
				event: { type: 'libraryCreate' }
			});

			platform.refreshMenuBar?.();

			navigate(`/${library.uuid}`);
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
