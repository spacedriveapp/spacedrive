import { useLibraryMutation, usePlausibleEvent, useZodForm } from '@sd/client';
import { Dialog, useDialog, UseDialogProps } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

interface Props extends UseDialogProps {
	onSuccess: () => void;
	locationId: number;
}

export default (props: Props) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const deleteLocation = useLibraryMutation('locations.delete', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'locationDelete' } });
			props.onSuccess();
		}
	});

	const form = useZodForm();

	const { t } = useLocale();

	return (
		<Dialog
			form={form}
			onSubmit={form.handleSubmit(() => deleteLocation.mutateAsync(props.locationId))}
			dialog={useDialog(props)}
			title={t('delete_location')}
			closeLabel={t('close')}
			icon={<Icon name="DeleteLocation" size={28} />}
			description={t('delete_location_description')}
			ctaDanger
			ctaLabel={t('delete')}
		/>
	);
};
