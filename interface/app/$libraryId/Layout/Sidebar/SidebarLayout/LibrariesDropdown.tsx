import { CloudArrowDown, Gear, Lock, Plus } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useClientContext } from '@sd/client';
import { dialogManager, Dropdown, DropdownMenu } from '@sd/ui';
import { useLocale } from '~/hooks';

import CreateDialog from '../../../settings/node/libraries/CreateDialog';
import { useSidebarContext } from './Context';
import JoinDialog from '~/app/$libraryId/settings/node/libraries/JoinDialog';

export default () => {
	const { library, libraries, currentLibraryId } = useClientContext();

	const sidebar = useSidebarContext();

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
						{libraries.isLoading
							? `${t('loading')}...`
							: library
								? library.config.name
								: ' '}
					</span>
				</Dropdown.Button>
			}
			// we override the sidebar dropdown item's hover styles
			// because the dark style clashes with the sidebar
			className="z-[100] mt-1 shadow-none data-[side=bottom]:slide-in-from-top-2 dark:divide-menu-selected/30 dark:border-sidebar-line dark:bg-sidebar-box"
			alignToTrigger
			// Timeout because of race conditions when opening the dropdown from a open popover.
			onOpenChange={(open) => setTimeout(() => sidebar.onLockedChange(open))}
		>
			{libraries.data
				?.map((lib) => (
					<DropdownMenu.Item
						to={`/${lib.uuid}`}
						key={lib.uuid}
						selected={lib.uuid === currentLibraryId}
					>
						<p className="truncate">{lib.config.name}</p>
					</DropdownMenu.Item>
				))
				.sort((a, b) => (a.props.selected ? -1 : 1))}
			<DropdownMenu.Separator className="mx-0" />
			<DropdownMenu.Item
				label={t('new_library')}
				icon={Plus}
				iconProps={{ weight: 'bold', size: 16 }}
				onClick={() => dialogManager.create((dp) => <CreateDialog {...dp} />)}
				className="font-medium"
			/>
			<DropdownMenu.Item
				label={t('join_library')}
				icon={CloudArrowDown}
				iconProps={{ weight: 'bold', size: 16 }}
				onClick={() => dialogManager.create((dp) => <JoinDialog librariesCtx={libraries.data} {...dp} />)}
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
