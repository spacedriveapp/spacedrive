import { useDebouncedCallback } from 'use-debounce';
import {
	GridViewSettings,
	ListViewSettings,
	MediaViewSettings,
	ViewSortBy,
	useLibraryMutation
} from '@sd/client';
import { RadixCheckbox, Select, SelectOption, Slider, tw } from '@sd/ui';
import { type SortOrder, SortOrderSchema } from '~/app/route-schemas';
import { getExplorerConfigStore, useExplorerConfigStore } from './config';
import {
	ExplorerLayoutMode,
	FilePathSearchOrderingKeys,
	getExplorerStore,
	useExplorerStore
} from './store';

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

//Brandon this is for you - these are the keys we have in the backend
//the values of the keys are shown in the dropdown

const dropDownMap: Record<ViewSortBy, string> = {
	None: 'None',
	Name: 'Name',
	Size: 'Size',
	DateCreated: 'Date created',
	DateModified: 'Date modified',
	DateIndexed: 'Date indexed',
	DateAccessed: 'Date accessed'
};

type LayoutSettings = {
	grid: GridViewSettings;
	list: ListViewSettings;
	media: MediaViewSettings;
};
type LayoutKeys<T extends keyof LayoutSettings> = LayoutSettings[T];

export default () => {
	const explorerStore = useExplorerStore();
	const explorerConfig = useExplorerConfigStore();

	//we only want to update if we are on a location page
	const locationUuid = getExplorerStore().locationUuid;
	const locationPreferences =
		getExplorerStore().viewLocationPreferences?.location?.[locationUuid];

	const { mutateAsync: updatePreferences } = useLibraryMutation('preferences.update', {
		onError: () => {
			alert('An error has occurred while updating your preferences.');
		}
	});

	const updatePreferencesHandler = useDebouncedCallback(
		async (
			layout: ExplorerLayoutMode,
			settingsToUpdate: Partial<LayoutKeys<ExplorerLayoutMode>>
		) => {
			if (!locationUuid) return;
			const updatedLocationPreferences = {
				[layout]: {
					...locationPreferences?.[layout],
					...settingsToUpdate
				}
			};
			await updatePreferences({
				location: {
					[locationUuid]: updatedLocationPreferences
				}
			});

			getExplorerStore().viewLocationPreferences = {
				location: {
					...explorerStore.viewLocationPreferences?.location,
					[locationUuid]: updatedLocationPreferences
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
								updatePreferencesHandler(explorerStore.layoutMode, {
									item_size: value[0] || 100
								});
							}}
							defaultValue={[
								locationPreferences?.grid?.item_size ?? explorerStore.gridItemSize
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
									updatePreferencesHandler(explorerStore.layoutMode, {
										item_size: 10 - val
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
							onChange={(value) => {
								getExplorerStore().orderBy = value as FilePathSearchOrderingKeys;
								updatePreferencesHandler(explorerStore.layoutMode, {
									sort_by: value as ViewSortBy
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
								getExplorerStore().orderByDirection = value as SortOrder;
								updatePreferencesHandler(explorerStore.layoutMode, {
									direction: value as SortOrder
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
							updatePreferencesHandler(explorerStore.layoutMode, {
								show_object_size: value
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
							updatePreferencesHandler(explorerStore.layoutMode, {
								show_square_thumbnails: value
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
						updatePreferencesHandler(explorerStore.layoutMode, {
							double_click_action: value === 'openFile'
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
