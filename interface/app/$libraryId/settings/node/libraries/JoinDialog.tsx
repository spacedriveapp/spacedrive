import {
	LibraryConfigWrapped,
	useBridgeMutation,
	useBridgeQuery,
	useClientContext,
	useLibraryContext,
	usePlausibleEvent,
	useZodForm
} from '@sd/client';
import { Button, Dialog, Select, SelectOption, toast, useDialog, UseDialogProps, z } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

const schema = z.object({
	libraryId: z.string().refine((value) => value !== 'select_library', {
		message: 'Please select a library'
	})
});


export default (props: UseDialogProps & { librariesCtx: LibraryConfigWrapped[] | undefined }) => {
	const cloudLibraries = useBridgeQuery(['cloud.library.list']);
	const joinLibrary = useBridgeMutation(['cloud.library.join']);

	const { t } = useLocale();
	const navigate = useNavigate();
	const platform = usePlatform();
	const queryClient = useQueryClient();

	const form = useZodForm({ schema, defaultValues: { libraryId: 'select_library' } });

	// const queryClient = useQueryClient();
	// const submitPlausibleEvent = usePlausibleEvent();
	// const platform = usePlatform();

	const onSubmit = form.handleSubmit(async (data) => {
		try {
			const library = await joinLibrary.mutateAsync(data.libraryId);

			queryClient.setQueryData(['library.list'], (libraries: any) => {
				// The invalidation system beat us to it
				if ((libraries || []).find((l: any) => l.uuid === library.uuid)) return libraries;

				return [...(libraries || []), library];
			});

			platform.refreshMenuBar && platform.refreshMenuBar();

			navigate(`/${library.uuid}`, { replace: true });
		} catch (e: any) {
			console.error(e);
			toast.error(e);
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={useDialog(props)}
			submitDisabled={!form.formState.isValid}
			title={t('join_library')}
			closeLabel={t('close')}
			cancelLabel={t('cancel')}
			description={t('join_library_description')}
			ctaLabel={form.formState.isSubmitting ? t('joining') : t('join')}
		>
			<div className="mt-5 space-y-4">
				{cloudLibraries.isLoading && <span>{t('loading')}...</span>}
				{cloudLibraries.data && (
					<Select
						value={form.watch('libraryId')}
						size="sm"
						className="w-full"
						onChange={(key) => {
							console.log('Key:', key);
							// Update the form value
							form.setValue('libraryId', key, {
								shouldValidate: true
							});
						}}
					>
						<SelectOption value="select_library">
							{t('select_library')}
						</SelectOption>
						{cloudLibraries.data
							.filter(
								(cloudLibrary) =>
									!props.librariesCtx?.find(
										(l: any) => l.uuid === cloudLibrary.uuid
									)
							)
							.map((cloudLibrary) => (
								<SelectOption key={cloudLibrary.uuid} value={cloudLibrary.uuid}>
									{cloudLibrary.name}
								</SelectOption>
							))}
					</Select>
				)}
			</div>
		</Dialog>
	);
};
