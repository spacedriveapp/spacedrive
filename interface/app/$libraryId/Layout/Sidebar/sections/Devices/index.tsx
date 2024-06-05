import { useState } from 'react';
import { HardwareModel, useBridgeQuery } from '@sd/client';
import { Button, toast, Tooltip, Dialog, Divider } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { hardwareModelToIcon } from '~/util/hardware';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import { useForm } from 'react-hook-form';
import { proxy } from 'valtio';

export default function DevicesSection() {
    const { data: node } = useBridgeQuery(['nodeState']);
    const { t } = useLocale();
    const [isDialogOpen, setIsDialogOpen] = useState(false);
    const form = useForm();
    const dialogState = proxy({ open: isDialogOpen });

    const handleOpenChange = (open: boolean) => {
        setIsDialogOpen(open);
        dialogState.open = open;
    };

    return (
        <Section name={t('devices')}>
            {node && (
                <SidebarLink className="group relative w-full" to={`node/${node.id}`} key={node.id}>
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
                onClick={() => handleOpenChange(true)}
                variant="dotted"
                className="mt-1 w-full opacity-70"
            >
                {t('add_device')}
            </Button>

            <Dialog
                form={form}
                dialog={{
                    id: 1,
                    state: dialogState,
                }}
                title="Add Device"
                description={t("Add Device Description")}
				icon={<Icon
					name={hardwareModelToIcon(node.device_model as HardwareModel)}
					size={28}
				/>}
                onSubmit={() => {
                    toast.info('Device added!');
                    handleOpenChange(false);
                }}
                ctaLabel="Add"
                closeLabel="Close"
                onCancelled={() => handleOpenChange(false)}
            >
                <div className="space-y-4 p-4">
                    <div className="flex flex-col items-center">
                        <div className="mb-4 size-32 rounded-lg bg-gray-600 shadow-lg" />
                    </div>
                    <div className="flex items-center space-x-2">
                        <Divider className="grow" />
                        <span className="text-ink-faint my-3">or</span>
                        <Divider className="grow" />
                    </div>
                    <div className="space-y-2">
                        <label htmlFor="accessCode" className="block text-sm text-gray-400">
                            Enter and authenticate device UUID
                        </label>
                        <input
                            id="accessCode"
                            className="block w-full rounded-md border border-gray-500 bg-[#272834] px-3 py-2 text-white shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-indigo-500 sm:text-sm"
                        />
                    </div>
                </div>
            </Dialog>
        </Section>
    );
}
