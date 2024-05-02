import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { insertLibrary, useBridgeMutation, usePlausibleEvent, useZodForm } from '@sd/client';
import { Dialog, InputField, useDialog, UseDialogProps, z } from '@sd/ui';
import { useLocale } from '~/hooks';
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
	const { t } = useLocale();

	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const submitPlausibleEvent = usePlausibleEvent();
	const platform = usePlatform();

	const createLibrary = useBridgeMutation('library.create');

	const form = useZodForm({ schema });

	const onSubmit = form.handleSubmit(async (data) => {
		try {
			const library = await createLibrary.mutateAsync({
				name: data.name,
				default_locations: null
			});

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
			title={t('create_new_library')}
			closeLabel={t('close')}
			cancelLabel={t('cancel')}
			description={t('create_new_library_description')}
			ctaLabel={form.formState.isSubmitting ? t('creating_library') : t('create_library')}
		>
			<div className="mt-5 space-y-4">
				<InputField
					{...form.register('name')}
					label={t('library_name')}
					placeholder={'e.g. "James\' Library"'}
					size="md"
				/>
			</div>
		</Dialog>
	);
};
