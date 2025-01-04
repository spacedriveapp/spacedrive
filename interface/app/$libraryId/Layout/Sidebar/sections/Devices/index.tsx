import { HardwareModel, useBridgeQuery, useLibraryQuery } from '@sd/client';
import { Button, dialogManager, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import AddDeviceDialog from './AddDeviceDialog';

export default function DevicesSection() {
	const { data: node } = useBridgeQuery(['nodeState']);
	const { data: devices } = useLibraryQuery(['devices.list'], {});
	const { t } = useLocale();

	return (
		<Section name={t('devices')}>
			{devices?.map((device) => (
				<SidebarLink
					className="group relative w-full"
					to={`node/${device.id}`}
					key={device.id as any}
				>
					{device.hardware_model ? (
						<Icon
							name={hardwareModelToIcon(device.hardware_model)}
							size={20}
							className="mr-1"
						/>
					) : (
						<Icon name="Laptop" className="mr-1" />
					)}

					<span className="truncate">{device.name}</span>
				</SidebarLink>
			))}
			<Tooltip label={t('devices_coming_soon_tooltip')} position="right">
				<Button
					disabled={!import.meta.env.DEV}
					onClick={() => dialogManager.create((dp) => <AddDeviceDialog {...dp} />)}
					variant="dotted"
					className="mt-1 w-full opacity-70"
				>
					{t('add_device')}
				</Button>
			</Tooltip>
		</Section>
	);
}
