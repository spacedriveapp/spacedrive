import { useForm } from 'react-hook-form';
import { HardwareModel, NodeState } from '@sd/client';
import { Dialog, Divider, Input, useDialog, UseDialogProps } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

interface Props extends UseDialogProps {
	node?: NodeState;
}

const AddDeviceDialog = ({ node, ...dialogProps }: Props) => {
	const form = useForm();
	const { t } = useLocale();

	return (
		<Dialog
			dialog={useDialog(dialogProps)}
			form={form}
			title={t('add_device')}
			description={t('Add Device Description')}
			icon={
				<Icon name={hardwareModelToIcon(node?.device_model as HardwareModel)} size={28} />
			}
			ctaLabel="Add"
			closeLabel="Close"
		>
			<div className="mt-4 flex flex-col items-center">
				<div className="size-32 rounded-lg bg-gray-600 shadow-lg" />
			</div>
			<div className="my-5 flex items-center space-x-3">
				<Divider className="grow" />
				<span className="my-1 text-xs text-ink-faint">OR</span>
				<Divider className="grow" />
			</div>
			<div className="space-y-2">
				<label htmlFor="accessCode" className="block text-sm text-gray-400">
					Enter and authenticate device UUID
				</label>
				<Input id="accessCode" />
			</div>
		</Dialog>
	);
};

export default AddDeviceDialog;
