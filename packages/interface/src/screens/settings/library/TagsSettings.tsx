import { useBridgeQuery, useLibraryCommand, useLibraryQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';
import React, { useState } from 'react';
import { HexColorPicker } from 'react-colorful';

import Card from '../../../components/layout/Card';
import Dialog from '../../../components/layout/Dialog';
import { PopoverPicker } from '../../../components/primitive/PopoverPicker';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function TagsSettings() {
	const [openCreateModal, setOpenCreateModal] = useState(false);
	const [color, setColor] = useState('#A717D9');
	const [name, setName] = useState('');

	const { data: tags } = useLibraryQuery('GetTags');

	const { mutate: createTag, isLoading } = useLibraryCommand('TagCreate', {
		onError: (e) => {
			console.log('error', e);
		},
		onSuccess: (data) => {
			setOpenCreateModal(false);
		}
	});

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Tags"
				description="Manage your tags."
				rightArea={
					<div className="flex-row space-x-2">
						<Dialog
							open={openCreateModal}
							onOpenChange={setOpenCreateModal}
							title="Create New Tag"
							description="Choose a name and color."
							ctaAction={() => {
								createTag({
									name,
									color
								});
							}}
							loading={isLoading}
							ctaLabel="Create"
							trigger={
								<Button variant="primary" size="sm">
									Create Tag
								</Button>
							}
						>
							<div className="relative mt-3">
								<PopoverPicker
									className="!absolute left-[9px] -top-[3px]"
									color={color}
									onChange={setColor}
								/>
								<Input
									value={name}
									onChange={(e) => setName(e.target.value)}
									className="w-full pl-[40px]"
									placeholder="Name"
								/>
							</div>
						</Dialog>
					</div>
				}
			/>

			<Card className="!px-2 dark:bg-gray-800">
				<div className="flex flex-wrap gap-2">
					{tags?.map((tag) => (
						<div
							key={tag.id}
							className="flex items-center rounded px-1.5 py-0.5"
							style={{ backgroundColor: tag.color + 'CC' }}
						>
							<span className="text-xs text-white drop-shadow-md">{tag.name}</span>
						</div>
					))}
				</div>
			</Card>
		</SettingsContainer>
	);
}
