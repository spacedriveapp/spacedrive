import {FolderOpen, GearSix, Planet, Plus, Robot} from '@phosphor-icons/react';
import type {Space} from '@sd/ts-client';
import {useNavigate} from 'react-router-dom';
import {useContextMenu} from '../../hooks/useContextMenu';
import {getLayoutItemKeyFromRoute, useExplorer} from '../../routes/explorer/context';
import {useGiveToAgentDialog} from '../modals/GiveToAgentModal';
import {SidebarItem} from '../Sidebar/SidebarItem';
import {SidebarSection} from '../Sidebar/SidebarSection';
import {useCreateSpaceDialog, useSpaceSettingsDialog} from './SpaceDialogs';
import {useSpaces} from './useSpaces';

function spaceRoute(space: Space, deviceSlug: string | undefined) {
	if (!space.path || !deviceSlug) return undefined;
	const path = {Physical: {device_slug: deviceSlug, path: space.path}};
	return `/explorer?path=${encodeURIComponent(JSON.stringify(path))}`;
}

function SpaceItem({space, deviceSlug}: {space: Space; deviceSlug?: string}) {
	const navigate = useNavigate();
	const {loadPreferencesForLayoutItem} = useExplorer();
	const openSettings = useSpaceSettingsDialog();
	const openGiveToAgent = useGiveToAgentDialog();
	const href = spaceRoute(space, deviceSlug);
	const select = () => {
		if (!href) return;
		const [pathname, query] = href.split('?');
		loadPreferencesForLayoutItem(getLayoutItemKeyFromRoute(pathname, `?${query}`));
	};
	const menu = useContextMenu({
		items: [
			{
				icon: FolderOpen,
				label: 'Open',
				onClick: () => {
					if (!href) return;
					select();
					navigate(href);
				},
				condition: () => !!href
			},
			{
				icon: Robot,
				label: 'Give to an Agent…',
				onClick: () => {
					if (space.path) openGiveToAgent({path: space.path, name: space.name});
				},
				condition: () => !!space.path
			},
			{type: 'separator'},
			{icon: GearSix, label: 'Space Settings…', onClick: () => openSettings(space)}
		]
	});

	return (
		<SidebarItem
			id={`space-${space.id}`}
			label={space.name}
			icon={Planet}
			href={href}
			disabled={!href}
			tone={href ? 'default' : 'muted'}
			title={space.path ?? `${space.name} is not on this device`}
			onSelect={select}
			onContextMenu={(event) => menu.show(event)}
		/>
	);
}

/** The library's spaces, with a button to start an empty workspace. */
export function SpacesSection({deviceSlug}: {deviceSlug?: string}) {
	const navigate = useNavigate();
	const {data} = useSpaces();
	const openCreate = useCreateSpaceDialog();
	const spaces = data?.spaces ?? [];

	return (
		<SidebarSection
			title="Spaces"
			action={
				<button
					type="button"
					title="New workspace"
					aria-label="New workspace"
					onClick={() =>
						openCreate({
							kind: 'workspace',
							onCreated: (space) => {
								const href = spaceRoute(space, deviceSlug);
								if (href) navigate(href);
							}
						})
					}
					className="text-sidebar-ink-faint hover:text-sidebar-ink-dull hover:bg-sidebar-box flex size-5 items-center justify-center rounded"
				>
					<Plus size={12} weight="bold" />
				</button>
			}
		>
			{spaces.map((space) => (
				<SpaceItem key={space.id} space={space} deviceSlug={deviceSlug} />
			))}
		</SidebarSection>
	);
}
