import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { HardwareModel, useBridgeMutation, useBridgeQuery, useZodForm } from '@sd/client';
import { Dialog, ErrorMessage, useDialog, UseDialogProps } from '@sd/ui';
import { Icon } from '~/components';
import { useAccessToken, useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';
import { usePlatform } from '~/util/Platform';

interface Props extends UseDialogProps {
	pubId: string;
	name: string;
	device_model: string;
}

interface CorePubId {
	Uuid: string;
}

export default function DeleteLibraryDialog(props: Props) {
	const { t } = useLocale();

	const queryClient = useQueryClient();
	const platform = usePlatform();
	const navigate = useNavigate();
	const accessToken = useAccessToken();
	const { data: node } = useBridgeQuery(['nodeState']);
	const deleteDevice = useBridgeMutation('cloud.devices.delete');
	const deviceAmount = useBridgeQuery(['cloud.devices.list']).data?.length;

	const form = useZodForm();

	// Check if the current device matches the UUID or if it's the only device
	useEffect(() => {
		if (deviceAmount === 1) {
			form.setError('pubId', {
				type: 'manual',
				message: t('error_only_device')
			});
		} else if ((node?.id as CorePubId).Uuid === props.pubId) {
			form.setError('pubId', {
				type: 'manual',
				message: t('error_current_device')
			});
		}
	}, [form, node, props.pubId, deviceAmount, t]);

	const onSubmit = form.handleSubmit(async () => {
		try {
			// Check for form errors before proceeding
			if (form.formState.errors.pubId) {
				return;
			}

			await deleteDevice.mutateAsync(props.pubId);
			queryClient.invalidateQueries({ queryKey: ['library.list'] });

			// eslint-disable-next-line @typescript-eslint/no-unused-expressions
			platform.refreshMenuBar && platform.refreshMenuBar();
			navigate('/');
		} catch (e) {
			alert(`Failed to delete device: ${e}`);
		}
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={useDialog(props)}
			title={t('delete_device')}
			closeLabel={t('close')}
			description={t('delete_device_description')}
			ctaDanger
			ctaLabel={t('delete')}
		>
			<div className="mt-5 flex flex-col items-center justify-center gap-2">
				<Icon
					// once backend endpoint is populated need to check if this is working correctly i.e fetching correct icons for devices
					name={hardwareModelToIcon(props.device_model as HardwareModel)}
					alt="Device icon"
					size={56}
					className="mr-2"
				/>
				<p className="text-medium mb-4 rounded-full text-sm font-medium">{props.name}</p>
				<ErrorMessage name="pubId" className="mb-4" />
			</div>
		</Dialog>
	);
}
