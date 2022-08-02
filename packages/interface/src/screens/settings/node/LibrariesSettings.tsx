import { TrashIcon } from '@heroicons/react/outline';
import { useBridgeMutation, useBridgeQuery } from '@sd/client';
import { LibraryConfigWrapped } from '@sd/core';
import { Button, Input } from '@sd/ui';
import { DotsSixVertical } from 'phosphor-react';
import React, { useState } from 'react';

import Card from '../../../components/layout/Card';
import Dialog from '../../../components/layout/Dialog';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

function LibraryListItem(props: { library: LibraryConfigWrapped }) {
	const [openDeleteModal, setOpenDeleteModal] = useState(false);

	const { mutate: deleteLib, isLoading: libDeletePending } = useBridgeMutation('library.delete', {
		onSuccess: () => {
			setOpenDeleteModal(false);
		}
	});

	return (
		<Card>
			<DotsSixVertical weight="bold" className="mt-[15px] mr-3 opacity-30" />
			<div className="flex-grow my-0.5">
				<h3 className="font-semibold">{props.library.config.name}</h3>
				<p className="mt-0.5 text-xs text-gray-200">{props.library.uuid}</p>
			</div>
			<div>
				<Dialog
					open={openDeleteModal}
					onOpenChange={setOpenDeleteModal}
					title="Delete Library"
					description="Deleting a library will permanently the database, the files themselves will not be deleted."
					ctaAction={() => {
						deleteLib(props.library.uuid);
					}}
					loading={libDeletePending}
					ctaDanger
					ctaLabel="Delete"
					trigger={
						<Button variant="gray" className="!p-1.5" onClick={() => {}}>
							<TrashIcon className="w-4 h-4" />
						</Button>
					}
				/>
			</div>
		</Card>
	);
}

export default function LibrarySettings() {
	const [openCreateModal, setOpenCreateModal] = useState(false);
	const [newLibName, setNewLibName] = useState('');
	const { data: libraries } = useBridgeQuery(['library.get']);
	const { mutate: createLibrary, isLoading: createLibLoading } = useBridgeMutation(
		'library.create',
		{
			onSuccess: () => {
				setOpenCreateModal(false);
			}
		}
	);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Libraries"
				description="The database contains all library data and file metadata."
				rightArea={
					<div className="flex-row space-x-2">
						<Dialog
							open={openCreateModal}
							onOpenChange={setOpenCreateModal}
							title="Create New Library"
							description="Choose a name for your new library, you can configure this and more settings from the library settings later on."
							ctaAction={() => createLibrary(newLibName)}
							loading={createLibLoading}
							submitDisabled={!newLibName}
							ctaLabel="Create"
							trigger={
								<Button variant="primary" size="sm">
									Add Library
								</Button>
							}
						>
							<Input
								className="flex-grow w-full mt-3"
								value={newLibName}
								placeholder="My Cool Library"
								onChange={(e) => setNewLibName(e.target.value)}
							/>
						</Dialog>
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
