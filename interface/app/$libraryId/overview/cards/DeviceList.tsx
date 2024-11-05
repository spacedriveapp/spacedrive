import { Desktop } from '@phosphor-icons/react';
import { useNavigate } from 'react-router';
import { Device, HardwareModel, useLibraryQuery } from '@sd/client';
import { Button, buttonStyles, Tooltip } from '@sd/ui';
import { Icon, Icon as SdIcon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

const DeviceList = () => {
	const navigate = useNavigate();
	const { t } = useLocale();

	const devicesQuery = useLibraryQuery(['devices.list'], {});
	const devices = devicesQuery.data ?? [];

	return (
		<>
			<div className="mb-2 flex items-center justify-between pt-1">
				<span className="text-sm font-medium">{t('Devices')}</span>
				<span className="text-xs text-ink-faint">{devices.length} total</span>
			</div>

			<div className="grid grid-cols-1 gap-2">
				{devices?.map((device) => (
					<button
						key={device.id}
						onClick={() => navigate(`settings/library/devices`)}
						className="flex items-center gap-3 rounded-md p-2.5 text-left hover:bg-app-selected/50"
					>
						{device.hardware_model ? (
							<Icon
								name={hardwareModelToIcon(
									device.hardware_model as unknown as HardwareModel
								)}
								size={30}
								className="mr-1"
							/>
						) : (
							<Icon name="Laptop" className="mr-1" />
						)}
						{device.name}
					</button>
				))}
			</div>
		</>
	);
};

export default DeviceList;
