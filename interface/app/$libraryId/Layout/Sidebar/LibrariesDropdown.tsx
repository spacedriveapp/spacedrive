import clsx from 'clsx';
import { Gear, Lock, Plus } from 'phosphor-react';
import { useClientContext } from '@sd/client';
import { Dropdown, dialogManager } from '@sd/ui';
import CreateDialog from '../../settings/node/libraries/CreateDialog';

export default () => {
	const { library, libraries, currentLibraryId } = useClientContext();

	return (
		<Dropdown.Root
			// we override the sidebar dropdown item's hover styles
			// because the dark style clashes with the sidebar
			itemsClassName="dark:bg-sidebar-box dark:border-sidebar-line mt-1 dark:divide-menu-selected/30 shadow-none"
			button={
				<Dropdown.Button
					variant="gray"
					className={clsx(
						`text-ink w-full `,
						// these classname overrides are messy
						// but they work
						`!bg-sidebar-box !border-sidebar-line/50 active:!border-sidebar-line active:!bg-sidebar-button ui-open:!bg-sidebar-button ui-open:!border-sidebar-line ring-offset-sidebar`,
						(library === null || libraries.isLoading) && '!text-ink-faint'
					)}
				>
					<span className="truncate">
						{libraries.isLoading ? 'Loading...' : library ? library.config.name : ' '}
					</span>
				</Dropdown.Button>
			}
		>
			<Dropdown.Section>
				{libraries.data?.map((lib) => (
					<Dropdown.Item
						to={`/${lib.uuid}/overview`}
						key={lib.uuid}
						selected={lib.uuid === currentLibraryId}
					>
						{lib.config.name}
					</Dropdown.Item>
				))}
			</Dropdown.Section>
			<Dropdown.Section>
				<Dropdown.Item
					icon={Plus}
					onClick={() => dialogManager.create((dp) => <CreateDialog {...dp} />)}
				>
					New Library
				</Dropdown.Item>
				<Dropdown.Item icon={Gear} to="settings/library">
					Manage Library
				</Dropdown.Item>
				<Dropdown.Item icon={Lock} onClick={() => alert('TODO: Not implemented yet!')}>
					Lock
				</Dropdown.Item>
			</Dropdown.Section>
		</Dropdown.Root>
	);
};
