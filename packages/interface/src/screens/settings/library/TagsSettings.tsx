import { Tag, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Card, Dialog, Switch } from '@sd/ui';
import clsx from 'clsx';
import { Trash } from 'phosphor-react';
import { useCallback, useEffect, useState } from 'react';
import { useDebounce } from 'rooks';
import { InputContainer } from '~/components/primitive/InputContainer';
import { PopoverPicker } from '~/components/primitive/PopoverPicker';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';

import { Form, Input, useZodForm, z } from '@sd/ui/src/forms';

export default function TagsSettings() {
	const [openCreateModal, setOpenCreateModal] = useState(false);
	const [openDeleteModal, setOpenDeleteModal] = useState(false);

	const { data: tags } = useLibraryQuery(['tags.list']);

	const [selectedTag, setSelectedTag] = useState<null | Tag>(tags?.[0] ?? null);

	const createTag = useLibraryMutation('tags.create', {
		onError: (e) => {
			console.error('error', e);
		}
	});
	const updateTag = useLibraryMutation('tags.update');
	const deleteTag = useLibraryMutation('tags.delete', {
		onSuccess: () => {
			setSelectedTag(null);
		}
	});

	const createForm = useZodForm({
		schema: z.object({
			name: z.string(),
			color: z.string()
		}),
		defaultValues: {
			color: '#A717D9'
		}
	});
	const updateForm = useZodForm({
		schema: z.object({
			id: z.number(),
			name: z.string().nullable(),
			color: z.string().nullable()
		}),
		defaultValues: selectedTag ?? undefined
	});
	const deleteForm = useZodForm({ schema: z.object({}) });

	const submitTagUpdate = updateForm.handleSubmit((data) => updateTag.mutateAsync(data));
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const autoUpdateTag = useCallback(useDebounce(submitTagUpdate, 500), [submitTagUpdate]);

	const setTag = useCallback(
		(tag: Tag | null) => {
			if (tag) updateForm.reset(tag);
			setSelectedTag(tag);
		},
		[setSelectedTag, updateForm]
	);

	useEffect(() => {
		const subscription = updateForm.watch(() => autoUpdateTag());
		return () => subscription.unsubscribe();
	}, [updateForm, autoUpdateTag]);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Tags"
				description="Manage your tags."
				rightArea={
					<div className="flex-row space-x-2">
						<Dialog
							form={createForm}
							onSubmit={createForm.handleSubmit(async (data) => {
								await createTag.mutateAsync(data);
								setOpenCreateModal(false);
							})}
							open={openCreateModal}
							setOpen={setOpenCreateModal}
							title="Create New Tag"
							description="Choose a name and color."
							loading={createTag.isLoading}
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
									{...createForm.register('color')}
								/>
								<Input
									{...createForm.register('name', { required: true })}
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
				<Form form={updateForm} onSubmit={submitTagUpdate}>
					<div className="flex flex-row mb-10 space-x-3">
						<div className="flex flex-col">
							<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
								Color
							</span>
							<div className="relative">
								<PopoverPicker
									className="!absolute left-[9px] -top-[3px]"
									{...updateForm.register('color')}
								/>
								<Input className="w-28 pl-[40px]" {...updateForm.register('color')} />
							</div>
						</div>
						<div className="flex flex-col">
							<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
								Name
							</span>
							<Input {...updateForm.register('name')} />
						</div>
						<div className="flex flex-grow" />
						<Dialog
							form={deleteForm}
							onSubmit={deleteForm.handleSubmit(async () => {
								await deleteTag.mutateAsync(selectedTag.id);
							})}
							open={openDeleteModal}
							setOpen={setOpenDeleteModal}
							title="Delete Tag"
							description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
							ctaDanger
							ctaLabel="Delete"
							trigger={
								<Button variant="gray" className="h-[38px] mt-[22px]">
									<Trash className="w-4 h-4" />
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
				</Form>
			) : (
				<div className="text-sm font-medium text-gray-400">No Tag Selected</div>
			)}
		</SettingsContainer>
	);
}
