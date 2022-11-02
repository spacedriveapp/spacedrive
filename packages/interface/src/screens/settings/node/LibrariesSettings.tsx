import { useBridgeMutation, useBridgeQuery, useCurrentLibrary } from '@sd/client';
import { LibraryConfigWrapped } from '@sd/client';
import { Button, ButtonLink, Card, tw } from '@sd/ui';
import { Database, DotsSixVertical, Link, Pen, Pencil, Trash } from 'phosphor-react';
import { useState } from 'react';

import CreateLibraryDialog from '../../../components/dialog/CreateLibraryDialog';
import DeleteLibraryDialog from '../../../components/dialog/DeleteLibraryDialog';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

const Pill = tw.span`px-1.5 ml-2 py-[2px] rounded text-xs font-medium bg-accent`;

function LibraryListItem(props: { library: LibraryConfigWrapped; current: boolean }) {
	const [openDeleteModal, setOpenDeleteModal] = useState(false);

	const deleteLibrary = useBridgeMutation('library.delete', {
		onSuccess: () => {
			setOpenDeleteModal(false);
		}
	});

	return (
		<Card>
			<DotsSixVertical weight="bold" className="mt-[15px] mr-3 opacity-30" />
			<div className="flex-1 my-0.5">
				<h3 className="font-semibold">
					{props.library.config.name}
					{props.current && <Pill>Current</Pill>}
				</h3>
				<p className="mt-0.5 text-xs text-ink-dull">{props.library.uuid}</p>
			</div>
			<div className="flex flex-row items-center space-x-2">
				<Button className="!p-1.5" onClick={() => {}} variant="gray">
					<Database className="w-4 h-4" />
				</Button>
				<ButtonLink className="!p-1.5" to="/settings/library" variant="gray">
					<Pencil className="w-4 h-4" />
				</ButtonLink>
				<DeleteLibraryDialog libraryUuid={props.library.uuid}>
					<Button className="!p-1.5" variant="gray">
						<Trash className="w-4 h-4" />
					</Button>
				</DeleteLibraryDialog>
			</div>
		</Card>
	);
}

export default function LibrarySettings() {
	const { data: libraries } = useBridgeQuery(['library.list']);
	const [open, setOpen] = useState(false);

	const { library: currentLibrary } = useCurrentLibrary();

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Libraries"
				description="The database contains all library data and file metadata."
				rightArea={
					<div className="flex-row space-x-2">
						<CreateLibraryDialog open={open} setOpen={setOpen}>
							<Button variant="accent" size="sm">
								Add Library
							</Button>
						</CreateLibraryDialog>
					</div>
				}
			/>

			<div className="space-y-2">
				{libraries
					?.sort((a, b) => {
						if (a.uuid === currentLibrary?.uuid) return -1;
						if (b.uuid === currentLibrary?.uuid) return 1;
						return 0;
					})
					.map((library) => (
						<LibraryListItem
							current={library.uuid === currentLibrary?.uuid}
							key={library.uuid}
							library={library}
						/>
					))}
			</div>
		</SettingsContainer>
	);
}
