import { useDebouncedCallback } from 'use-debounce';
import { stringify } from 'uuid';
import { ExplorerSettings, useLibraryMutation } from '@sd/client';
import { RadixCheckbox, Select, SelectOption, Slider, tw } from '@sd/ui';
import { SortOrderSchema } from '~/app/route-schemas';
import { useExplorerContext } from './Context';
import { getExplorerConfigStore, useExplorerConfigStore } from './config';
import { FilePathSearchOrderingKeys, getExplorerStore, useExplorerStore } from './store';

const Subheading = tw.div`text-ink-dull mb-1 text-xs font-medium`;

//these are the keys we have here in the frontend

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
	const explorerConfig = useExplorerConfigStore();
	const explorerContext = useExplorerContext();
	const locationUuid =
		explorerContext.parent?.type === 'Location'
			? stringify(explorerContext.parent.location.pub_id)
			: '';
	const locationExplorerSettings =
		getExplorerStore().viewLocationPreferences?.location?.[locationUuid]?.explorer;

	const updatePreferences = useLibraryMutation('preferences.update', {
		onError: () => {
			alert('An error has occurred while updating your preferences.');
		}
	});

	const updatePreferencesHandler = useDebouncedCallback(
		async (settingsToUpdate: Partial<ExplorerSettings>) => {
			const locationSettings =
				getExplorerStore().viewLocationPreferences?.location?.[locationUuid];

			if (!locationSettings) return;

			const updatedExplorerSettings: ExplorerSettings = {
				...locationSettings.explorer,
				...settingsToUpdate
			};

			await updatePreferences.mutateAsync({
				location: {
					[locationUuid]: {
						...locationSettings,
						explorer: updatedExplorerSettings
					}
				}
			});

			getExplorerStore().viewLocationPreferences = {
				location: {
					[locationUuid]: {
						...locationSettings,
						explorer: updatedExplorerSettings
					}
				}
			};
		},
		300
	);

	return (
		<div className="flex w-80 flex-col gap-4 p-4">
			{(explorerStore.layoutMode === 'grid' || explorerStore.layoutMode === 'media') && (
				<div>
					<Subheading>Item size</Subheading>
					{explorerStore.layoutMode === 'grid' ? (
						<Slider
							onValueChange={(value) => {
								getExplorerStore().gridItemSize = value[0] || 100;
								updatePreferencesHandler({
									itemSize: value[0] || 100
								});
							}}
							defaultValue={[
								locationExplorerSettings?.itemSize ?? explorerStore.gridItemSize
							]}
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
									getExplorerStore().mediaColumns = 10 - val;
									updatePreferencesHandler({
										itemSize: 10 - val
									});
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
								getExplorerStore().orderBy = sortBy;
								updatePreferencesHandler({
									sortBy
								});
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
								getExplorerStore().orderByDirection = value;
								updatePreferencesHandler({
									direction: value
								});
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
							getExplorerStore().showBytesInGridView = value;
							updatePreferencesHandler({
								showSize: value
							});
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
							getExplorerStore().mediaAspectSquare = value;
							updatePreferencesHandler({
								mediaSqrThumbs: value
							});
						}
					}}
					className="mt-1"
				/>
			)}
			<div>
				<Subheading>Double click action</Subheading>
				<Select
					className="w-full"
					value={explorerConfig.openOnDoubleClick ? 'openFile' : 'quickPreview'}
					onChange={(value) => {
						getExplorerConfigStore().openOnDoubleClick = value === 'openFile';
						updatePreferencesHandler({
							// this should really be an enum
							dblClickAction: value === 'openFile'
						});
					}}
				>
					<SelectOption value="openFile">Open File</SelectOption>
					<SelectOption value="quickPreview">Quick Preview</SelectOption>
				</Select>
			</div>
		</div>
	);
};
