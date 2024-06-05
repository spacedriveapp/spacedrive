import { HardwareModel, useBridgeQuery } from '@sd/client';
import { Button, dialogManager } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import AddDeviceDialog from './AddDeviceDialog';

export default function DevicesSection() {
    const { data: node } = useBridgeQuery(['nodeState']);
    const { t } = useLocale();

    return (
        <Section name={t('devices')}>
            {node && (
                <SidebarLink className="relative w-full group" to={`node/${node.id}`} key={node.id}>
                    {node.device_model ? (
                        <Icon
                            name={hardwareModelToIcon(node.device_model as HardwareModel)}
                            size={20}
                            className="mr-1"
                        />
                    ) : (
                        <Icon name="Laptop" className="mr-1" />
                    )}

                    <span className="truncate">{node.name}</span>
                </SidebarLink>
            )}
            <Button
                onClick={() => dialogManager.create((dp) => (
					<AddDeviceDialog {...dp} />
				))}
                variant="dotted"
                className="w-full mt-1 opacity-70"
            >
                {t('add_device')}
            </Button>
        </Section>
    );
}
