import {
	createOrdering,
	explorerLayout,
	getOrderingDirection,
	getOrderingKey,
	useExplorerLayoutStore
} from '@sd/client';
import { RadixCheckbox, Select, SelectOption, Slider, tw, z } from '@sd/ui';
import i18n from '~/app/I18n';
import { SortOrderSchema } from '~/app/route-schemas';
import { useLocale } from '~/hooks';

import { useExplorerContext } from '../Context';
import { ListViewOptions } from './ListView';

export const Subheading = tw.div`text-ink-dull mb-1 text-xs font-medium`;

export default () => {
	const { t } = useLocale();

	const explorer = useExplorerContext();
	const layoutStore = useExplorerLayoutStore();
	const settings = explorer.useSettingsSnapshot();

	return (
		<div className="flex w-80 flex-col gap-4 p-4">
			<div className="grid grid-cols-2 gap-2">
				<div className="flex flex-col">
					<Subheading>{t('sort_by')}</Subheading>
					<Select
						value={settings.order ? getOrderingKey(settings.order) : 'none'}
						size="sm"
						className="w-full"
						onChange={(key) => {
							if (key === 'none') explorer.settingsStore.order = null;
							else
								explorer.settingsStore.order = createOrdering(
									key,
									explorer.settingsStore.order
										? getOrderingDirection(explorer.settingsStore.order)
										: 'Asc'
								);
						}}
					>
						<SelectOption value="none">{t('none')}</SelectOption>
						{explorer.orderingKeys?.options.map((option) => (
							<SelectOption key={option.value} value={option.value}>
								{t(`${option.description?.toLowerCase().split(' ').join('_')}`)}
							</SelectOption>
						))}
					</Select>
				</div>

				<div className="flex flex-col">
					<Subheading>{t('direction')}</Subheading>
					<Select
						value={settings.order ? getOrderingDirection(settings.order) : 'Asc'}
						size="sm"
						className="w-full"
						disabled={explorer.settingsStore.order === null}
						onChange={(order) => {
							if (explorer.settingsStore.order === null) return;

							explorer.settingsStore.order = createOrdering(
								getOrderingKey(explorer.settingsStore.order),
								order
							);
						}}
					>
						{SortOrderSchema.options.map((o) => (
							<SelectOption key={o.value} value={o.value}>
								{o.description}
							</SelectOption>
						))}
					</Select>
				</div>
			</div>

			{settings.layoutMode === 'media' && (
				<div>
					<Subheading>{t('media_view_context')}</Subheading>
					<Select
						className="w-full"
						value={
							explorer.settingsStore.mediaViewWithDescendants
								? 'withDescendants'
								: 'withoutDescendants'
						}
						onChange={(value) => {
							explorer.settingsStore.mediaViewWithDescendants =
								value === 'withDescendants';
						}}
					>
						{mediaViewContextActions.options.map((option) => (
							<SelectOption key={option.value} value={option.value}>
								{option.description}
							</SelectOption>
						))}
					</Select>
				</div>
			)}

			{(settings.layoutMode === 'grid' || settings.layoutMode === 'media') && (
				<div>
					<Subheading>{t('item_size')}</Subheading>
					{settings.layoutMode === 'grid' ? (
						<Slider
							onValueChange={(value) => {
								explorer.settingsStore.gridItemSize = value[0] || 100;
							}}
							value={[settings.gridItemSize]}
							max={200}
							step={10}
							min={60}
						/>
					) : (
						<Slider
							defaultValue={[10 - settings.mediaColumns]}
							min={0}
							max={6}
							step={1}
							onValueChange={([val]) => {
								if (val !== undefined)
									explorer.settingsStore.mediaColumns = 10 - val;
							}}
						/>
					)}
				</div>
			)}

			{settings.layoutMode === 'grid' && (
				<div>
					<Subheading>{t('grid_gap')}</Subheading>
					<Slider
						onValueChange={([val]) => {
							if (val) explorer.settingsStore.gridGap = val;
						}}
						defaultValue={[settings.gridGap]}
						max={16}
						min={4}
						step={4}
					/>
				</div>
			)}

			{settings.layoutMode === 'list' && <ListViewOptions />}

			<div>
				<Subheading>{t('double_click_action')}</Subheading>
				<Select
					className="w-full"
					value={settings.openOnDoubleClick}
					onChange={(value) => {
						explorer.settingsStore.openOnDoubleClick = value;
					}}
				>
					{doubleClickActions.options.map((option) => (
						<SelectOption key={option.value} value={option.value}>
							{option.description}
						</SelectOption>
					))}
				</Select>
			</div>

			<div>
				<Subheading>{t('explorer')}</Subheading>
				<div className="grid grid-cols-2 gap-y-1">
					<RadixCheckbox
						checked={layoutStore.showPathBar}
						label={t('show_path_bar')}
						name="showPathBar"
						onCheckedChange={(value) => {
							if (typeof value !== 'boolean') return;
							explorerLayout.showPathBar = value;
						}}
					/>
					<RadixCheckbox
						checked={layoutStore.showTags}
						label={t('show_tags')}
						name="showTags"
						onCheckedChange={(value) => {
							if (typeof value !== 'boolean') return;
							explorerLayout.showTags = value;
						}}
					/>

					{settings.layoutMode === 'grid' && (
						<RadixCheckbox
							checked={settings.showBytesInGridView}
							label={t('show_object_size')}
							name="showBytesInGridView"
							onCheckedChange={(value) => {
								if (typeof value !== 'boolean') return;

								explorer.settingsStore.showBytesInGridView = value;
							}}
						/>
					)}

					<RadixCheckbox
						checked={settings.showHiddenFiles}
						label={t('show_hidden_files')}
						name="showHiddenFiles"
						onCheckedChange={(value) => {
							if (typeof value !== 'boolean') return;
							explorer.settingsStore.showHiddenFiles = value;
						}}
					/>

					{settings.layoutMode === 'media' && (
						<RadixCheckbox
							checked={settings.mediaAspectSquare}
							label={t('square_thumbnails')}
							name="mediaAspectSquare"
							onCheckedChange={(value) => {
								if (typeof value !== 'boolean') return;

								explorer.settingsStore.mediaAspectSquare = value;
							}}
						/>
					)}
				</div>
			</div>
		</div>
	);
};

const doubleClickActions = z.union([
	z.literal('openFile').describe(i18n.t('open_file')),
	z.literal('quickPreview').describe(i18n.t('quick_preview'))
]);

const mediaViewContextActions = z.union([
	z.literal('withDescendants').describe(i18n.t('current_directory_with_descendants')),
	z.literal('withoutDescendants').describe(i18n.t('current_directory'))
]);
