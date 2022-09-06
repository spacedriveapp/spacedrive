import { PencilIcon, TrashIcon } from '@heroicons/react/24/outline';
import { useBridgeMutation, useBridgeQuery, useLibraryStore } from '@sd/client';
import { LibraryConfigWrapped } from '@sd/core';
import { Button, Input } from '@sd/ui';
import { DotsSixVertical } from 'phosphor-react';
import React, { useState } from 'react';
import { useNavigate } from 'react-router';

import CreateLibraryDialog from '../../../components/dialog/CreateLibraryDialog';
import DeleteLibraryDialog from '../../../components/dialog/DeleteLibraryDialog';
import Card from '../../../components/layout/Card';
import Dialog from '../../../components/layout/Dialog';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

function LibraryListItem(props: { library: LibraryConfigWrapped }) {
	const navigate = useNavigate();
	const [openDeleteModal, setOpenDeleteModal] = useState(false);

	const { currentLibraryUuid, switchLibrary } = useLibraryStore();

	const { mutate: deleteLib, isLoading: libDeletePending } = useBridgeMutation('library.delete', {
		onSuccess: () => {
			setOpenDeleteModal(false);
		}
	});

	function handleEditLibrary() {
		// switch library if requesting to edit non-current library
		navigate('/settings/library');
		if (props.library.uuid !== currentLibraryUuid) {
			switchLibrary(props.library.uuid);
		}
	}

	return (
		<Card>
			<DotsSixVertical weight="bold" className="mt-[15px] mr-3 opacity-30" />
			<div className="flex-grow my-0.5">
				<h3 className="font-semibold">{props.library.config.name}</h3>
				<p className="mt-0.5 text-xs text-gray-200">{props.library.uuid}</p>
			</div>
			<div className="mt-2 space-x-2">
				<Button variant="gray" className="!p-1.5" onClick={handleEditLibrary}>
					<PencilIcon className="w-4 h-4" />
				</Button>
				<DeleteLibraryDialog libraryUuid={props.library.uuid}>
					<Button variant="gray" className="!p-1.5">
						<TrashIcon className="w-4 h-4" />
					</Button>
				</DeleteLibraryDialog>
			</div>
		</Card>
	);
}

export default function LibrarySettings() {
	const { data: libraries } = useBridgeQuery(['library.get']);

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
