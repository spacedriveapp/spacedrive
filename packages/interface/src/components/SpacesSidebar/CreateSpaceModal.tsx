import { useState } from 'react';
import clsx from 'clsx';
import { Input, Label, dialogManager, useDialog, Dialog } from '@sd/ui';
import { useLibraryMutation } from '@sd/ts-client';
import { useForm } from 'react-hook-form';

interface FormData {
	name: string;
}

const PRESET_COLORS = [
	'#3B82F6', // Blue
	'#8B5CF6', // Purple
	'#EC4899', // Pink
	'#10B981', // Green
	'#F59E0B', // Amber
	'#EF4444', // Red
	'#06B6D4', // Cyan
	'#6366F1', // Indigo
];

const PRESET_ICONS = [
	'Planet',
	'Folder',
	'Briefcase',
	'House',
	'Camera',
	'MusicNotes',
	'GameController',
	'Code',
];

export function useCreateSpaceDialog() {
	return dialogManager.create((props) => <CreateSpaceDialog {...props} />);
}

function CreateSpaceDialog(props: { id: number }) {
	const dialog = useDialog(props);
	const [selectedColor, setSelectedColor] = useState(PRESET_COLORS[0]);
	const [selectedIcon, setSelectedIcon] = useState(PRESET_ICONS[0]);

	const form = useForm<FormData>({
		defaultValues: { name: '' },
	});

	const createSpace = useLibraryMutation('spaces.create');

	const onSubmit = form.handleSubmit(async (data) => {
		if (!data.name?.trim()) return;

		await createSpace.mutateAsync({
			name: data.name,
			icon: selectedIcon,
			color: selectedColor,
		});
		form.reset();
		setSelectedColor(PRESET_COLORS[0]);
		setSelectedIcon(PRESET_ICONS[0]);
		dialog.state.open = false;
	});

	return (
		<Dialog
			form={form}
			dialog={dialog}
			title="Create Space"
			onSubmit={onSubmit}
			ctaLabel="Create"
		>
			<div className="space-y-4">
				<div>
					<Label>Space Name</Label>
					<Input
						{...form.register('name', { required: true })}
						placeholder="e.g., Work Files, Personal Photos"
						autoFocus
					/>
				</div>

				<div>
					<Label>Color</Label>
					<div className="flex flex-wrap gap-2">
						{PRESET_COLORS.map((color) => (
							<button
								key={color}
								type="button"
								onClick={() => setSelectedColor(color)}
								className={clsx(
									'h-8 w-8 rounded-full border-2 transition-all',
									selectedColor === color
										? 'scale-110 border-white'
										: 'border-transparent'
								)}
								style={{ backgroundColor: color }}
							/>
						))}
					</div>
				</div>

				<div>
					<Label>Icon</Label>
					<div className="flex flex-wrap gap-2">
						{PRESET_ICONS.map((icon) => (
							<button
								key={icon}
								type="button"
								onClick={() => setSelectedIcon(icon)}
								className={clsx(
									'rounded-lg px-3 py-2 text-sm font-medium transition-colors',
									selectedIcon === icon
										? 'bg-sidebar-selected text-sidebar-ink'
										: 'bg-app-input text-sidebar-ink-dull hover:bg-app-hover'
								)}
							>
								{icon}
							</button>
						))}
					</div>
				</div>
			</div>
		</Dialog>
	);
}
