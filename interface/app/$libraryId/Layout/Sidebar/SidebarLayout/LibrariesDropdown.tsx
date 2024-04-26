import { Gear, Lock, Plus } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useClientContext } from '@sd/client';
import { dialogManager, Dropdown, DropdownMenu } from '@sd/ui';
import { useLocale } from '~/hooks';

import CreateDialog from '../../../settings/node/libraries/CreateDialog';

export default () => {
	const { library, libraries, currentLibraryId } = useClientContext();

	const { t } = useLocale();

	return (
		<DropdownMenu.Root
			trigger={
				<Dropdown.Button
					variant="gray"
					className={clsx(
						`w-full text-sidebar-ink`,
						// these classname overrides are messy
						// but they work
						`!border-sidebar-line/50 !bg-sidebar-box ring-offset-sidebar active:!border-sidebar-line active:!bg-sidebar-button ui-open:!border-sidebar-line ui-open:!bg-sidebar-button`,
						(library === null || libraries.isLoading) && '!text-sidebar-inkFaint'
					)}
				>
					<span className="truncate">
						{libraries.isLoading ? 'Loading...' : library ? library.config.name : ' '}
					</span>
				</Dropdown.Button>
			}
			// we override the sidebar dropdown item's hover styles
			// because the dark style clashes with the sidebar
			className="mt-1 shadow-none data-[side=bottom]:slide-in-from-top-2 dark:divide-menu-selected/30 dark:border-sidebar-line dark:bg-sidebar-box"
			alignToTrigger
		>
			{libraries.data?.map((lib) => (
				<DropdownMenu.Item
					to={`/${lib.uuid}`}
					key={lib.uuid}
					selected={lib.uuid === currentLibraryId}
				>
					<p className="truncate">{lib.config.name}</p>
				</DropdownMenu.Item>
			))}
			<DropdownMenu.Separator className="mx-0" />
			<DropdownMenu.Item
				label={t('new_library')}
				icon={Plus}
				iconProps={{ weight: 'bold', size: 16 }}
				onClick={() => dialogManager.create((dp) => <CreateDialog {...dp} />)}
				className="font-medium"
			/>
			<DropdownMenu.Item
				label={t('manage_library')}
				icon={Gear}
				iconProps={{ weight: 'bold', size: 16 }}
				to="settings/library/general"
				className="font-medium"
			/>
			{/* <DropdownMenu.Item
				label={t('lock')}
				icon={Lock}
				iconProps={{ weight: 'bold', size: 16 }}
				onClick={() => alert('TODO: Not implemented yet!')}
				className="font-medium"
			/> */}
		</DropdownMenu.Root>
	);
};
