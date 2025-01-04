import { ArrowRight } from '@phosphor-icons/react';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import {
	CloudDevice,
	CloudP2PNotifyUser,
	CloudP2PTicket,
	CloudSyncGroupWithDevices,
	HardwareModel,
	useBridgeMutation,
	useZodForm
} from '@sd/client';
import { Dialog, toast, useDialog, UseDialogProps, z } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';
import { usePlatform } from '~/util/Platform';

type ReceivedJoinRequest = Extract<CloudP2PNotifyUser, { kind: 'ReceivedJoinSyncGroupRequest' }>;

export default (
	props: {
		data: ReceivedJoinRequest['data'];
	} & UseDialogProps
) => {
	// PROPS = device_name, device_model, library_name
	// you will probably have to change the props to accept the library id and device id to pair them properly. Omitted for now as
	// unsure what the data will look like when the backend is populated

	// const joinLibrary = useBridgeMutation(['cloud.library.join']);

	const { t } = useLocale();
	const navigate = useNavigate();
	const platform = usePlatform();
	const queryClient = useQueryClient();

	const form = useZodForm({ defaultValues: { libraryId: 'select_library' } });
	const userResponse = useBridgeMutation('cloud.userResponse');

	// adapted from another dialog - we can change the form submit/remove form if needed but didn't want to
	// unnecessarily remove code
	const onSubmit = form.handleSubmit(async (_d) => {
		try {
			// const library = await joinLibrary.mutateAsync(data.libraryId);
			userResponse.mutate({
				kind: 'AcceptDeviceInSyncGroup',
				data: {
					ticket: props.data.ticket,
					accepted: {
						id: props.data.sync_group.library.pub_id,
						name: props.data.sync_group.library.name,
						description: null
					}
				}
			});

			queryClient.setQueryData(['library.list'], (libraries: any) => {
				// The invalidation system beat us to it
				if (
					(libraries || []).find(
						(l: any) => l.uuid === props.data.sync_group.library.pub_id
					)
				)
					return libraries;

				return [...(libraries || []), props.data.sync_group.library];
			});

			if (platform.refreshMenuBar) platform.refreshMenuBar();

			navigate(`/${props.data.sync_group.library.pub_id}`, { replace: true });
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
			title={t('request_add_device')}
			cancelLabel={t('cancel')}
			cancelDanger={true}
			cancelBtn={true}
			onCancelled={false}
			description={t('request_add_device_description')}
			ignoreClickOutside={true}
			ctaLabel={form.formState.isSubmitting ? t('accepting') : t('accept')}
		>
			{/* device */}
			<div className="my-6 flex items-center justify-center gap-10">
				<div className="flex flex-col items-center justify-center gap-2">
					<Icon
						// once backend endpoint is populated need to check if this is working correctly i.e fetching correct icons for devices
						name={hardwareModelToIcon(props.data.asking_device.hardware_model)}
						alt="Device icon"
						size={48}
						className="mr-2"
					/>
					<p className="text-sm text-ink-dull">{props.data.asking_device.name}</p>
				</div>
				<ArrowRight color="#ABACBA" size={18}></ArrowRight>
				{/* library */}
				<div className="flex flex-col items-center justify-center gap-2">
					<Icon
						// once backend endpoint is populated need to check if this is working correctly i.e fetching correct icons for devices
						name={'Book'}
						alt="Device icon"
						size={48}
						className="mr-2"
					/>
					<p className="text-sm text-ink-dull">{props.data.sync_group.library.name}</p>
				</div>
			</div>
		</Dialog>
	);
};
