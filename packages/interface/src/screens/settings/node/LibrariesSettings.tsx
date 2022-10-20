import { PencilIcon, TrashIcon } from '@heroicons/react/24/outline';
import { useBridgeMutation, useBridgeQuery } from '@sd/client';
import { LibraryConfigWrapped } from '@sd/client';
import { Button, ButtonLink } from '@sd/ui';
import { DotsSixVertical } from 'phosphor-react';
import { useState } from 'react';

import CreateLibraryDialog from '../../../components/dialog/CreateLibraryDialog';
import DeleteLibraryDialog from '../../../components/dialog/DeleteLibraryDialog';
import Card from '../../../components/layout/Card';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

function LibraryListItem(props: { library: LibraryConfigWrapped }) {
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
				<h3 className="font-semibold">{props.library.config.name}</h3>
				<p className="mt-0.5 text-xs text-gray-200">{props.library.uuid}</p>
			</div>
			<div className="flex flex-row space-x-2 items-center">
				<ButtonLink to="/settings/library" variant="gray" padding="sm">
					<PencilIcon className="w-4 h-4" />
				</ButtonLink>
				<DeleteLibraryDialog libraryUuid={props.library.uuid}>
					<Button variant="gray" padding="sm">
						<TrashIcon className="w-4 h-4" />
					</Button>
				</DeleteLibraryDialog>
			</div>
		</Card>
	);
}

export default function LibrarySettings() {
	const { data: libraries } = useBridgeQuery(['library.list']);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Libraries"
				description="The database contains all library data and file metadata."
				rightArea={
					<div className="flex-row space-x-2">
						<CreateLibraryDialog>
							<Button variant="primary" size="sm">
								Add Library
							</Button>
						</CreateLibraryDialog>
					</div>
				}
			/>

			<div className="space-y-2">
				{libraries?.map((library) => (
					<LibraryListItem key={library.uuid} library={library} />
				))}
			</div>
		</SettingsContainer>
	);
}
