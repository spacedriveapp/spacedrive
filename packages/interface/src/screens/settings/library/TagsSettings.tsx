import { TrashIcon } from '@heroicons/react/24/outline';
import { Tag, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { TagUpdateArgs } from '@sd/client';
import { Button, Card, Dialog, Input, Switch } from '@sd/ui';
import clsx from 'clsx';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { useDebounce } from 'rooks';

import { InputContainer } from '../../../components/primitive/InputContainer';
import { PopoverPicker } from '../../../components/primitive/PopoverPicker';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function TagsSettings() {
	const [openCreateModal, setOpenCreateModal] = useState(false);
	const [openDeleteModal, setOpenDeleteModal] = useState(false);
	// creating new tag state
	const [newColor, setNewColor] = useState('#A717D9');
	const [newName, setNewName] = useState('');

	const { data: tags } = useLibraryQuery(['tags.list']);

	const [selectedTag, setSelectedTag] = useState<null | Tag>(tags?.[0] ?? null);

	const { mutate: createTag, isLoading } = useLibraryMutation('tags.create', {
		onError: (e) => {
			console.error('error', e);
		},
		onSuccess: (_) => {
			setOpenCreateModal(false);
		}
	});

	const updateTag = useLibraryMutation('tags.update');

	const deleteTag = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			setSelectedTag(null);
		}
	});

	const { register, handleSubmit, watch, reset, control } = useForm({
		defaultValues: selectedTag as TagUpdateArgs
	});

	const setTag = useCallback(
		(tag: Tag | null) => {
			if (tag) reset(tag);
			setSelectedTag(tag);
		},
		[setSelectedTag, reset]
	);

	const submitTagUpdate = handleSubmit((data) => updateTag.mutate(data));
	// eslint-disable-next-line react-hooks/exhaustive-deps
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
							setOpen={setOpenCreateModal}
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
								<Button variant="accent" size="sm">
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
			<Card className="!px-2">
				<div className="flex flex-wrap gap-2 m-1">
					{tags?.map((tag) => (
						<div
							onClick={() => setTag(tag.id === selectedTag?.id ? null : tag)}
							key={tag.id}
							className={clsx(
								'flex items-center rounded px-1.5 py-0.5',
								selectedTag?.id === tag.id && 'ring'
							)}
							style={{ backgroundColor: tag.color + 'CC' }}
						>
							<span className="text-xs text-white drop-shadow-md">{tag.name}</span>
						</div>
					))}
				</div>
			</Card>
			{selectedTag ? (
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
						<div className="flex flex-col">
							<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
								Name
							</span>
							<Input {...register('name')} />
						</div>
						<div className="flex flex-grow" />
						<Dialog
							open={openDeleteModal}
							setOpen={setOpenDeleteModal}
							title="Delete Tag"
							description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
							ctaAction={() => {
								deleteTag.mutate(selectedTag.id);
							}}
							loading={deleteTag.isLoading}
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
						<Switch checked />
					</InputContainer>
				</form>
			) : (
				<div className="text-sm font-medium text-gray-400">No Tag Selected</div>
			)}
		</SettingsContainer>
	);
}
