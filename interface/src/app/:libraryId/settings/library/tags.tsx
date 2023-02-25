import clsx from 'clsx';
import { Trash } from 'phosphor-react';
import { useCallback, useEffect, useState } from 'react';
import { useDebounce } from 'rooks';
import { Tag, useLibraryMutation, useLibraryQuery } from '@sd/client';
import {
	Button,
	Card,
	Dialog,
	Switch,
	Tooltip,
	UseDialogProps,
	dialogManager,
	useDialog
} from '@sd/ui';
import { Form, Input, useZodForm, z } from '@sd/ui/src/forms';
import { PopoverPicker } from '~/components/primitive/PopoverPicker';
import { Heading } from '../Layout';
import Setting from '../Setting';

export default function TagsSettings() {
	const tags = useLibraryQuery(['tags.list']);

	const [selectedTag, setSelectedTag] = useState<null | Tag>(tags.data?.[0] ?? null);

	const updateTag = useLibraryMutation('tags.update');

	const updateForm = useZodForm({
		schema: z.object({
			id: z.number(),
			name: z.string().nullable(),
			color: z.string().nullable()
		}),
		defaultValues: selectedTag ?? undefined
	});

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
		<>
			<Heading
				title="Tags"
				description="Manage your tags."
				rightArea={
					<div className="flex-row space-x-2">
						<Button
							variant="accent"
							size="sm"
							onClick={() => {
								dialogManager.create((dp) => <CreateTagDialog {...dp} />);
							}}
						>
							Create Tag
						</Button>
					</div>
				}
			/>
			<Card className="!px-2">
				<div className="m-1 flex flex-wrap gap-2">
					{tags.data?.map((tag) => (
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
					<div className="mb-10 flex flex-row space-x-3">
						<div className="flex flex-col">
							<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
								Color
							</span>
							<div className="relative">
								<PopoverPicker
									className="!absolute left-[9px] top-[-5px]"
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
						<div className="flex grow" />
						<Button
							variant="gray"
							className="mt-[22px] h-[38px]"
							onClick={() =>
								dialogManager.create((dp) => (
									<DeleteTagDialog
										{...dp}
										tagId={selectedTag.id}
										onSuccess={() => setSelectedTag(null)}
									/>
								))
							}
						>
							<Tooltip label="Delete Tag">
								<Trash className="h-4 w-4" />
							</Tooltip>
						</Button>
					</div>
					<Setting mini title="Show in Spaces" description="Show this tag on the spaces screen.">
						<Switch checked />
					</Setting>
				</Form>
			) : (
				<div className="text-sm font-medium text-gray-400">No Tag Selected</div>
			)}
		</>
	);
}

function CreateTagDialog(props: UseDialogProps) {
	const dialog = useDialog(props);

	const form = useZodForm({
		schema: z.object({
			name: z.string(),
			color: z.string()
		}),
		defaultValues: {
			color: '#A717D9'
		}
	});

	const createTag = useLibraryMutation('tags.create', {
		onError: (e) => {
			console.error('error', e);
		}
	});

	return (
		<Dialog
			{...{ dialog, form }}
			onSubmit={form.handleSubmit((data) => createTag.mutateAsync(data))}
			title="Create New Tag"
			description="Choose a name and color."
			ctaLabel="Create"
		>
			<div className="relative mt-3 ">
				<PopoverPicker className="!absolute left-[9px] top-[-5px]" {...form.register('color')} />
				<Input
					{...form.register('name', { required: true })}
					className="w-full pl-[40px]"
					placeholder="Name"
				/>
			</div>
		</Dialog>
	);
}

interface DeleteTagDialogProps extends UseDialogProps {
	tagId: number;
	onSuccess: () => void;
}

function DeleteTagDialog(props: DeleteTagDialogProps) {
	const dialog = useDialog(props);

	const form = useZodForm({ schema: z.object({}) });

	const deleteTag = useLibraryMutation('tags.delete', {
		onSuccess: props.onSuccess
	});

	return (
		<Dialog
			{...{ form, dialog }}
			onSubmit={form.handleSubmit(() => deleteTag.mutateAsync(props.tagId))}
			title="Delete Tag"
			description="Are you sure you want to delete this tag? This cannot be undone and tagged files will be unlinked."
			ctaDanger
			ctaLabel="Delete"
		/>
	);
}
