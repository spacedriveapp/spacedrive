import { useLibraryMutation, usePlausibleEvent, useZodForm } from '@sd/client';
import { Dialog, useDialog, UseDialogProps } from '@sd/ui';
import { useLocale } from '~/hooks';

interface Props extends UseDialogProps {
	tagId: number;
	onSuccess: () => void;
}

export default (props: Props) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const deleteTag = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagDelete' } });
			props.onSuccess();
		}
	});

	const form = useZodForm();

	const { t } = useLocale();

	return (
		<Dialog
			form={form}
			dialog={useDialog(props)}
			onSubmit={form.handleSubmit(() => deleteTag.mutateAsync(props.tagId))}
			title="Delete Tag"
			description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
			ctaDanger
			ctaLabel={t('delete')}
		/>
	);
};
