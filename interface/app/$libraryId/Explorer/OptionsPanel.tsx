import { useDebouncedCallback } from 'use-debounce';
import { stringify } from 'uuid';
import { useLibraryMutation } from '@sd/client';
import { RadixCheckbox, Select, SelectOption, Slider, tw } from '@sd/ui';
import { SortOrderSchema } from '~/app/route-schemas';
import { useExplorerContext } from './Context';
import {
	FilePathSearchOrderingKeys,
	defaultExplorerSettings,
	getExplorerSettings,
	getExplorerStore,
	useExplorerStore
} from './store';

const Subheading = tw.div`text-ink-dull mb-1 text-xs font-medium`;

export const sortOptions: Record<FilePathSearchOrderingKeys, string> = {
	'none': 'None',
	'name': 'Name',
	'sizeInBytes': 'Size',
	'dateCreated': 'Date created',
	'dateModified': 'Date modified',
	'dateIndexed': 'Date indexed',
	'object.dateAccessed': 'Date accessed'
};

export default () => {
	const explorerStore = useExplorerStore();
	const explorerContext = useExplorerContext();
	const locationUuid =
		explorerContext.parent?.type === 'Location'
			? stringify(explorerContext.parent.location.pub_id)
			: '';

	const updatePreferences = useLibraryMutation('preferences.update', {
		onError: () => {
			alert('An error has occurred while updating your preferences.');
		}
	});

	const updatePreferencesHandler = useDebouncedCallback(
		async (
			settingToUpdate: keyof typeof defaultExplorerSettings,
			value: (typeof defaultExplorerSettings)[keyof typeof defaultExplorerSettings]
		) => {
			const updatedExplorerSettings = {
				...getExplorerSettings(),
				[settingToUpdate]: value
			};
			await updatePreferences.mutateAsync({
				location: {
					[locationUuid]: {
						explorer: updatedExplorerSettings
					}
				}
			});
		},
		100
	);

	return (
		<div className="flex flex-col gap-4 p-4 w-80">
			{(explorerStore.layoutMode === 'grid' || explorerStore.layoutMode === 'media') && (
				<div>
					<Subheading>Item size</Subheading>
					{explorerStore.layoutMode === 'grid' ? (
						<Slider
							onValueChange={(value) => {
								if (!locationUuid)
									return (getExplorerStore().gridItemSize = value[0] || 100);
								updatePreferencesHandler('gridItemSize', value[0] || 100);
								getExplorerStore().gridItemSize = value[0] || 100;
							}}
							defaultValue={[explorerStore.gridItemSize]}
							max={200}
							step={10}
							min={60}
						/>
					) : (
						<Slider
							defaultValue={[10 - explorerStore.mediaColumns]}
							min={0}
							max={6}
							step={2}
							onValueChange={([val]) => {
								if (val !== undefined) {
									if (!locationUuid)
										return (getExplorerStore().mediaColumns = 10 - val);
									updatePreferencesHandler('mediaColumns', 10 - val);
									getExplorerStore().mediaColumns = 10 - val;
								}
							}}
						/>
					)}
				</div>
			)}
			{(explorerStore.layoutMode === 'grid' || explorerStore.layoutMode === 'media') && (
				<div className="grid grid-cols-2 gap-2">
					<div className="flex flex-col">
						<Subheading>Sort by</Subheading>
						<Select
							value={explorerStore.orderBy}
							size="sm"
							className="w-full"
							onChange={(sortBy) => {
								if (!locationUuid) return (getExplorerStore().orderBy = sortBy);
								updatePreferencesHandler('orderBy', sortBy);
								getExplorerStore().orderBy = sortBy;
							}}
						>
							{Object.entries(sortOptions).map(([value, text]) => (
								<SelectOption key={value} value={value}>
									{text}
								</SelectOption>
							))}
						</Select>
					</div>

					<div className="flex flex-col">
						<Subheading>Direction</Subheading>
						<Select
							value={explorerStore.orderByDirection}
							size="sm"
							className="w-full"
							onChange={(value) => {
								if (!locationUuid)
									return (getExplorerStore().orderByDirection = value);
								updatePreferencesHandler('orderByDirection', value);
								getExplorerStore().orderByDirection = value;
							}}
						>
							{SortOrderSchema.options.map((o) => (
								<SelectOption key={o.value} value={o.value}>
									{o.value}
								</SelectOption>
							))}
						</Select>
					</div>
				</div>
			)}

			{explorerStore.layoutMode === 'grid' && (
				<RadixCheckbox
					checked={explorerStore.showBytesInGridView}
					label="Show Object size"
					name="showBytesInGridView"
					onCheckedChange={(value) => {
						if (typeof value === 'boolean') {
							if (!locationUuid)
								return (getExplorerStore().showBytesInGridView = value);
							updatePreferencesHandler('showBytesInGridView', value);
							getExplorerStore().showBytesInGridView = value;
						}
					}}
					className="mt-1"
				/>
			)}

			{explorerStore.layoutMode === 'media' && (
				<RadixCheckbox
					checked={explorerStore.mediaAspectSquare}
					label="Show square thumbnails"
					name="mediaAspectSquare"
					onCheckedChange={(value) => {
						if (typeof value === 'boolean') {
							if (!locationUuid)
								return (getExplorerStore().mediaAspectSquare = value);
							updatePreferencesHandler('mediaAspectSquare', value);
							getExplorerStore().mediaAspectSquare = value;
						}
					}}
					className="mt-1"
				/>
			)}
			<div>
				<Subheading>Double click action</Subheading>
				<Select
					className="w-full"
					value={explorerStore.openOnDoubleClick}
					onChange={(value) => {
						if (!locationUuid) return (getExplorerStore().openOnDoubleClick = value);
						updatePreferencesHandler('openOnDoubleClick', value);
						getExplorerStore().openOnDoubleClick = value;
					}}
				>
					<SelectOption value="openFile">Open File</SelectOption>
					<SelectOption value="quickPreview">Quick Preview</SelectOption>
				</Select>
			</div>
		</div>
	);
};
