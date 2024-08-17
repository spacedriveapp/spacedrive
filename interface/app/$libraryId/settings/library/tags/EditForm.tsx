import { Trash } from '@phosphor-icons/react';
import { Tag, useLibraryMutation, useZodForm } from '@sd/client';
import { Button, dialogManager, Form, InputField, Switch, Tooltip, z } from '@sd/ui';
import { ColorPicker } from '~/components';
import { useDebouncedFormWatch, useLocale } from '~/hooks';

import Setting from '../../Setting';
import DeleteDialog from './DeleteDialog';

const schema = z.object({
	name: z.string().trim().min(1).max(24).nullable(),
	color: z
		.string()
		.trim()
		.min(2)
		.max(7)
		.regex(/^#([0-9A-F]{1}){1,6}$/i, 'Invalid hex color')
		.nullable()
});

interface Props {
	tag: Tag;
	onDelete: () => void;
}

export default ({ tag, onDelete }: Props) => {
	const updateTag = useLibraryMutation('tags.update');

	const { t } = useLocale();

	const form = useZodForm({
		schema,
		mode: 'onChange',
		defaultValues: tag,
		reValidateMode: 'onChange'
	});

	useDebouncedFormWatch(form, (data) => {
		updateTag.mutate({
			name: data.name ?? null,
			color: data.color ?? null,
			id: tag.id
		});
	});

	return (
		<Form form={form}>
			<div className="mb-10 flex items-end justify-between">
				<div className="flex flex-row space-x-3">
					<InputField
						label={t('color')}
						maxLength={7}
						value={form.watch('color')?.trim() ?? '#ffffff'}
						icon={<ColorPicker control={form.control} name="color" />}
						{...form.register('color')}
					/>

					<InputField maxLength={24} label={t('name')} {...form.register('name')} />
				</div>
				<Button
					variant="gray"
					className="flex size-[30px] items-center justify-center"
					onClick={() =>
						dialogManager.create((dp) => (
							<DeleteDialog {...dp} tagId={tag.id} onSuccess={onDelete} />
						))
					}
				>
					<Tooltip label={t('delete_tag')}>
						<Trash className="size-4" />
					</Tooltip>
				</Button>
			</div>
			{/* <div className="flex flex-col gap-2">
				<Setting
					mini
					title={t('hide_in_library_search')}
					description={t('hide_in_library_search_description')}
				>
					<Switch disabled />
				</Setting>
				<Setting
					mini
					title={t('hide_in_sidebar')}
					description={t('hide_in_sidebar_description')}
				>
					<Switch disabled />
				</Setting>
			</div> */}
		</Form>
	);
};
