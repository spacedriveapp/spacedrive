import { Trash } from '@phosphor-icons/react';
import { iconNames } from '@sd/assets/util';
import { Key } from 'react';
import { HardwareModel, humanizeSize } from '@sd/client';
import { Button, Card, dialogManager, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useAccessToken, useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import DeleteDeviceDialog from './DeleteDeviceDialog';

interface DeviceItemProps {
	pub_id: Key | null | undefined;
	name: string;
	os: string;
	device_model: string;
	storage_size: bigint;
	used_storage: bigint;
	created_at: string;
}

// unsure where to put pub_id/if this information is important for a user? also have not included used_storage
export default (props: DeviceItemProps) => {
	const { t } = useLocale();

	return (
		<Card className="flex min-w-96 items-center space-x-3 p-2">
			<Icon
				// once backend endpoint is populated need to check if this is working correctly i.e fetching correct icons for devices
				name={hardwareModelToIcon(props.device_model as HardwareModel)}
				alt="Device icon"
				size={24}
				className="mr-2"
			/>
			<div className="flex-1">
				<h4 className="text-sm font-semibold">{props.name}</h4>
				<p className="text-xs text-ink-dull">
					{props.os}, {`${t('added')}`} {new Date(props.created_at).toLocaleDateString()}
				</p>
				<p className="text-xs text-ink-dull"></p>
			</div>
			<div className="text-xs text-ink-dull">{`${humanizeSize(props.storage_size)}`}</div>
			<Button
				className="!p-1"
				variant="gray"
				onClick={() => {
					// add delete device functionality when avail
				}}
			>
				<Tooltip label={t('Delete device')}>
					<Trash
						onClick={() => {
							dialogManager.create((dp) => (
								<DeleteDeviceDialog
									name={props.name}
									device_model={props.device_model}
									pubId={String(props.pub_id)}
									{...dp}
								/>
							));
						}}
						className="size-4"
					/>
				</Tooltip>
			</Button>
		</Card>
	);
};
