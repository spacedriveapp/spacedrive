import {Planet} from '@phosphor-icons/react';
import type {File} from '@sd/ts-client';
import {Button} from '@spacedrive/primitives';
import {InfoRow, Section} from '../Inspector/Inspector';
import {useSpaceSettingsDialog} from './SpaceDialogs';
import {useSpaceAt} from './useSpaces';

/** A folder's space, when it is the root of one, with its settings. */
export function SpaceInspectorSection({file}: {file: File}) {
	const path =
		file.kind === 'Directory' && file.is_local && 'Physical' in file.sd_path
			? file.sd_path.Physical.path
			: null;
	const space = useSpaceAt(path);
	const openSettings = useSpaceSettingsDialog();
	if (!space) return null;

	return (
		<Section title="Space" icon={Planet}>
			<InfoRow label="Name" value={space.name} />
			<InfoRow
				label="Kind"
				value={space.kind === 'workspace' ? 'Workspace' : 'Folder'}
			/>
			<InfoRow
				label="Content indexing"
				value={space.content_indexing ? 'On' : 'Off'}
			/>
			<InfoRow label="Versioning" value="Off" />
			<Button
				type="button"
				variant="gray"
				size="sm"
				className="w-full"
				onClick={() => openSettings(space)}
			>
				Space settings…
			</Button>
		</Section>
	);
}
