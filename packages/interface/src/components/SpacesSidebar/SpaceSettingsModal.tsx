import { useState } from 'react';
import clsx from 'clsx';
import type { Space } from '@sd/ts-client';
import { Input, Label, dialogManager, useDialog, Dialog } from '@spacedrive/primitives';
import { useLibraryMutation } from '@sd/ts-client';
import { useForm } from 'react-hook-form';
import { PRESET_COLORS, PRESET_ICONS } from './spacePresets';

interface FormData {
	name: string;
}

export function useSpaceSettingsDialog(space: Space) {
	return dialogManager.create((props) => (
		<SpaceSettingsDialog {...props} space={space} />
	));
}

function SpaceSettingsDialog(props: { id: number; space: Space }) {
	const dialog = useDialog(props);
	const [selectedColor, setSelectedColor] = useState(props.space.color);
	const [selectedIcon, setSelectedIcon] = useState(props.space.icon);

	const form = useForm<FormData>({
		defaultValues: { name: props.space.name },
	});

	const updateSpace = useLibraryMutation('spaces.update');

	const onSubmit = form.handleSubmit(async (data) => {
		if (!data.name?.trim()) return;

		await updateSpace.mutateAsync({
			space_id: props.space.id,
			name: data.name.trim(),
			icon: selectedIcon,
			color: selectedColor,
		});
		dialog.state.open = false;
	});

	return (
		<Dialog
			form={form}
			dialog={dialog}
			title="Space Settings"
			onSubmit={onSubmit}
			ctaLabel="Save"
		>
			<div className="space-y-4">
				<div>
					<Label>Space Name</Label>
					<Input
						{...form.register('name', { required: true })}
						placeholder="e.g., All Devices, Work Files"
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
										: 'border-transparent',
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
										: 'bg-app-input text-sidebar-ink-dull hover:bg-app-hover',
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