import { HardwareModel, NodeState } from '@sd/client';
import { Dialog, Divider, Input, UseDialogProps, useDialog } from '@sd/ui';
import { Icon } from '~/components';
import { hardwareModelToIcon } from '~/util/hardware';

import { useForm } from 'react-hook-form';
import { useLocale } from '~/hooks';

interface Props extends UseDialogProps {
	node?: NodeState
}

const AddDeviceDialog = ({node, ...dialogProps}: Props) => {

	const form = useForm();
	const { t } = useLocale();

	return (
		<Dialog
		dialog={useDialog(dialogProps)}
		form={form}
		title="Add Device"
		description={t("Add Device Description")}
		icon={<Icon
			name={hardwareModelToIcon(node?.device_model as HardwareModel)}
			size={28}
		/>}
		ctaLabel="Add"
		closeLabel="Close"
		>
			<div className="flex flex-col items-center mt-4">
				<div className="bg-gray-600 rounded-lg shadow-lg size-32" />
			</div>
			<div className="flex items-center my-5 space-x-3">
				<Divider className="grow" />
				<span className="my-1 text-xs text-ink-faint">OR</span>
				<Divider className="grow" />
			</div>
			<div className="space-y-2">
				<label htmlFor="accessCode" className="block text-sm text-gray-400">
					Enter and authenticate device UUID
				</label>
				<Input id="accessCode"/>
			</div>
	</Dialog>
	);
};

export default AddDeviceDialog;
