import { CaretDown, CaretRight, Pencil, Trash } from '@phosphor-icons/react';
import { AnimatePresence, motion } from 'framer-motion';
import { Key, useState } from 'react';
import { LibraryConfigWrapped, useBridgeQuery } from '@sd/client';
import { Button, ButtonLink, Card, dialogManager, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

import DeleteDialog from './DeleteDialog';
import DeviceItem from './DeviceItem';

interface Props {
	library: LibraryConfigWrapped;
	current: boolean;
}

export default (props: Props) => {
	const { t } = useLocale();
	const [isExpanded, setIsExpanded] = useState(false);

	const cloudDevicesList = useBridgeQuery(['cloud.devices.list'], {
		suspense: true,
		retry: false
	});

	const toggleExpansion = () => {
		setIsExpanded((prev) => !prev);
	};

	return (
		<div>
			<Card className="items-center justify-between">
				<div className="flex cursor-pointer items-center">
					<Icon name="Database" alt="Database icon" size={30} className="mr-3" />
					<div className="my-0.5 flex-1">
						<h3 className="font-semibold">
							{props.library.config.name}
							{props.current && (
								<span className="ml-2 rounded bg-accent px-1.5 py-[2px] text-xs font-medium text-white">
									{t('current')}
								</span>
							)}
						</h3>
						<p className="mt-0.5 text-xs text-ink-dull">{props.library.uuid}</p>
					</div>
				</div>
				<div className="flex flex-row items-center space-x-2">
					<ButtonLink
						className="!p-1.5"
						to={`/${props.library.uuid}/settings/library/general`}
						variant="gray"
					>
						<Tooltip label={t('edit_library')}>
							<Pencil className="size-4" />
						</Tooltip>
					</ButtonLink>
					<Button
						className="!p-1.5"
						variant="gray"
						onClick={() => {
							dialogManager.create((dp) => (
								<DeleteDialog {...dp} libraryUuid={props.library.uuid} />
							));
						}}
					>
						<Tooltip label={t('delete_library')}>
							<Trash className="size-4" />
						</Tooltip>
					</Button>
					<Button onClick={toggleExpansion} className="!p-1.5" variant="gray">
						<Tooltip label={t('toggle devices')}>
							<motion.div
								animate={{ rotate: isExpanded ? 90 : 0 }}
								transition={{ duration: 0.2 }}
							>
								<CaretRight size={16} className="ml-auto" />
							</motion.div>
						</Tooltip>
					</Button>
				</div>
			</Card>

			<AnimatePresence>
				{isExpanded && (
					<motion.div
						initial={{ height: 0, opacity: 0 }}
						animate={{ height: 'auto', opacity: 1 }}
						exit={{ height: 0, opacity: 0 }}
						className="relative mt-2 flex origin-top flex-col gap-1 pl-8"
					>
						<div className="absolute inset-y-0 left-6 mb-7 w-[2px] rounded-t-full bg-[#5E5F69] "></div>

						{cloudDevicesList.data?.map(
							(
								device: {
									pub_id: Key | null | undefined;
									name: string;
									os: string;
									storage_size: number;
									created_at: string;
								},
								index: number
							) => (
								<div key={device.pub_id} className="relative flex items-center">
									<motion.div
										initial={{ opacity: 0, y: 10 }}
										animate={{ opacity: 1, y: 0 }}
										transition={{ delay: index * 0.03 }}
										className="relative flex w-full items-center"
									>
										<div className="absolute left-[-0.5em] right-6 h-[2px] w-11 rounded-r-full bg-[#5E5F69]"></div>
										<div className="flex-1 pl-12">
											<DeviceItem
												pub_id={device.pub_id}
												name={device.name}
												os={device.os}
												storage_size={device.storage_size}
												created_at={device.created_at}
											/>
										</div>
									</motion.div>
								</div>
							)
						)}
					</motion.div>
				)}
			</AnimatePresence>
		</div>
	);
};
