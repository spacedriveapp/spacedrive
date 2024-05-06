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
			title={t('delete_tag')}
			closeLabel={t('close')}
			description={t('delete_tag_description')}
			ctaDanger
			ctaLabel={t('delete')}
		/>
	);
};
