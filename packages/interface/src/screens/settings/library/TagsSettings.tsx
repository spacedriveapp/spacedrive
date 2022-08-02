import { TrashIcon } from '@heroicons/react/outline';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { TagUpdateArgs } from '@sd/core';
import { Button, Input } from '@sd/ui';
import clsx from 'clsx';
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { useDebounce } from 'rooks';

import Card from '../../../components/layout/Card';
import Dialog from '../../../components/layout/Dialog';
import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { PopoverPicker } from '../../../components/primitive/PopoverPicker';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function TagsSettings() {
	const [openCreateModal, setOpenCreateModal] = useState(false);
	// creating new tag state
	const [newColor, setNewColor] = useState('#A717D9');
	const [newName, setNewName] = useState('');

	const { data: tags } = useLibraryQuery(['tags.get']);

	const [selectedTag, setSelectedTag] = useState<null | number>(null);

	const currentTag = useMemo(() => {
		return tags?.find((t) => t.id === selectedTag);
	}, [tags, selectedTag]);

	const { mutate: createTag, isLoading } = useLibraryMutation('tags.create', {
		onError: (e) => {
			console.log('error', e);
		},
		onSuccess: (data) => {
			setOpenCreateModal(false);
		}
	});

	const { mutate: updateTag } = useLibraryMutation('tags.update');

	const { mutate: deleteTag, isLoading: tagDeleteLoading } = useLibraryMutation('tags.delete');

	// set default selected tag
	useEffect(() => {
		if (!currentTag && tags?.length) {
			setSelectedTag(tags[0].id);
		}
	}, [tags]);

	useEffect(() => {
		reset(currentTag);
	}, [currentTag]);

	const { register, handleSubmit, watch, reset, control } = useForm({
		defaultValues: currentTag as TagUpdateArgs
	});

	const submitTagUpdate = handleSubmit((data) => updateTag(data));

	const autoUpdateTag = useCallback(useDebounce(submitTagUpdate, 500), []);

	useEffect(() => {
		const subscription = watch(() => autoUpdateTag());
		return () => subscription.unsubscribe();
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
									name: newName,
									color: newColor
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
							<div className="relative mt-3 ">
								<PopoverPicker
									className="!absolute left-[9px] -top-[3px]"
									value={newColor}
									onChange={setNewColor}
								/>
								<Input
									value={newName}
									onChange={(e) => setNewName(e.target.value)}
									className="w-full pl-[40px]"
									placeholder="Name"
								/>
							</div>
						</Dialog>
					</div>
				}
			/>

			<Card className="!px-2 dark:bg-gray-800">
				<div className="flex flex-wrap gap-2 m-1">
					{tags?.map((tag) => (
						<div
							onClick={() => setSelectedTag(tag.id === selectedTag ? null : tag.id)}
							key={tag.id}
							className={clsx(
								'flex items-center rounded px-1.5 py-0.5',
								selectedTag === tag.id && 'ring'
							)}
							style={{ backgroundColor: tag.color + 'CC' }}
						>
							<span className="text-xs text-white drop-shadow-md">{tag.name}</span>
						</div>
					))}
				</div>
			</Card>
			{currentTag ? (
				<form onSubmit={submitTagUpdate}>
					<div className="flex flex-row mb-10 space-x-3">
						<div className="flex flex-col">
							<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
								Color
							</span>
							<div className="relative">
								<Controller
									name="color"
									control={control}
									render={({ field: { onChange, value } }) => (
										<PopoverPicker
											className="!absolute left-[9px] -top-[3px]"
											value={value || ''}
											onChange={onChange}
										/>
									)}
								/>

								<Input className="w-28 pl-[40px]" {...register('color')} />
							</div>
						</div>
						<div className="flex flex-col ">
							<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
								Name
							</span>
							<Input {...register('name')} />
						</div>
						<div className="flex flex-grow"></div>
						<Dialog
							title="Delete Tag"
							description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
							ctaAction={() => {
								deleteTag(currentTag.id);
							}}
							loading={tagDeleteLoading}
							ctaDanger
							ctaLabel="Delete"
							trigger={
								<Button variant="gray" className="h-[38px] mt-[22px]">
									<TrashIcon className="w-4 h-4" />
								</Button>
							}
						/>
					</div>
					<InputContainer
						mini
						title="Show in Spaces"
						description="Show this tag on the spaces screen."
					>
						<Toggle value />
					</InputContainer>
				</form>
			) : (
				<div className="text-sm font-medium text-gray-400">No Tag Selected</div>
			)}
		</SettingsContainer>
	);
}
