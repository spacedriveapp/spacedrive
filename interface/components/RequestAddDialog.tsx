import { ArrowRight } from '@phosphor-icons/react';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router';
import { HardwareModel, useBridgeMutation, useZodForm } from '@sd/client';
import { Dialog, toast, useDialog, UseDialogProps, z } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';
import { usePlatform } from '~/util/Platform';

export default (
	props: {
		device_name: string;
		device_model: HardwareModel;
		library_name: string;
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

	// adapted from another dialog - we can change the form submit/remove form if needed but didn't want to
	// unnecessarily remove code
	const onSubmit = form.handleSubmit(async (data) => {
		try {
			// const library = await joinLibrary.mutateAsync(data.libraryId);
			const library = { uuid: '1234' }; // dummy data

			queryClient.setQueryData(['library.list'], (libraries: any) => {
				// The invalidation system beat us to it
				if ((libraries || []).find((l: any) => l.uuid === library.uuid)) return libraries;

				return [...(libraries || []), library];
			});

			if (platform.refreshMenuBar) platform.refreshMenuBar();

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
						name={hardwareModelToIcon(props.device_model)}
						alt="Device icon"
						size={48}
						className="mr-2"
					/>
					<p className="text-sm text-ink-dull">{props.device_name}</p>
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
					<p className="text-sm text-ink-dull">{props.library_name}</p>
				</div>
			</div>
		</Dialog>
	);
};
