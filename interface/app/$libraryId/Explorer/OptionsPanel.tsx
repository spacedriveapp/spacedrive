import { useMatch } from 'react-router';
import { RadixCheckbox, Select, SelectOption, Slider, tw, z } from '@sd/ui';
import { getExplorerLayoutStore, useExplorerLayoutStore } from '~/../packages/client/src';
import { SortOrderSchema } from '~/app/route-schemas';

import { useExplorerContext } from './Context';
import {
	createOrdering,
	getExplorerStore,
	getOrderingDirection,
	orderingKey,
	useExplorerStore
} from './store';

const Subheading = tw.div`text-ink-dull mb-1 text-xs font-medium`;

export default () => {
	const explorerStore = useExplorerStore();
	const explorer = useExplorerContext();
	const layoutStore = useExplorerLayoutStore();

	const settings = explorer.useSettingsSnapshot();

	const isEphemeralLocation = useMatch('/:libraryId/ephemeral/:ephemeralId');

	return (
		<div className="flex w-80 flex-col gap-4 p-4">
			{(settings.layoutMode === 'grid' || settings.layoutMode === 'media') && (
				<div>
					<Subheading>Item size</Subheading>
					{settings.layoutMode === 'grid' ? (
						<Slider
							onValueChange={(value) => {
								explorer.settingsStore.gridItemSize = value[0] || 100;
							}}
							defaultValue={[settings.gridItemSize]}
							max={200}
							step={10}
							min={60}
						/>
					) : (
						<Slider
							defaultValue={[10 - settings.mediaColumns]}
							min={0}
							max={6}
							step={2}
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
					<Subheading>Gap</Subheading>
					<Slider
						onValueChange={([val]) => {
							if (val) getExplorerStore().gridGap = val;
						}}
						defaultValue={[explorerStore.gridGap]}
						max={16}
						min={4}
						step={4}
					/>
				</div>
			)}

			{(settings.layoutMode === 'grid' || settings.layoutMode === 'media') && (
				<div className="grid grid-cols-2 gap-2">
					<div className="flex flex-col">
						<Subheading>Sort by</Subheading>
						<Select
							value={settings.order ? orderingKey(settings.order) : 'none'}
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
							<SelectOption value="none">None</SelectOption>
							{explorer.orderingKeys?.options.map((option) => (
								<SelectOption key={option.value} value={option.value}>
									{option.description}
								</SelectOption>
							))}
						</Select>
					</div>

					<div className="flex flex-col">
						<Subheading>Direction</Subheading>
						<Select
							value={settings.order ? getOrderingDirection(settings.order) : 'Asc'}
							size="sm"
							className="w-full"
							disabled={explorer.settingsStore.order === null}
							onChange={(order) => {
								if (explorer.settingsStore.order === null) return;

								explorer.settingsStore.order = createOrdering(
									orderingKey(explorer.settingsStore.order),
									order
								);
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

			<div>
				<Subheading>Explorer</Subheading>
				<div className="grid grid-cols-2 gap-y-1">
					<RadixCheckbox
						checked={layoutStore.showPathBar}
						label="Show Path Bar"
						name="showPathBar"
						onCheckedChange={(value) => {
							if (typeof value !== 'boolean') return;
							getExplorerLayoutStore().showPathBar = value;
						}}
					/>

					{settings.layoutMode === 'grid' && !isEphemeralLocation && (
						<RadixCheckbox
							checked={settings.showBytesInGridView}
							label="Show Object size"
							name="showBytesInGridView"
							onCheckedChange={(value) => {
								if (typeof value !== 'boolean') return;

								explorer.settingsStore.showBytesInGridView = value;
							}}
						/>
					)}

					<RadixCheckbox
						checked={settings.showHiddenFiles}
						label="Show Hidden Files"
						name="showHiddenFiles"
						onCheckedChange={(value) => {
							if (typeof value !== 'boolean') return;

							explorer.settingsStore.showHiddenFiles = value;
						}}
					/>

					{settings.layoutMode === 'media' && (
						<RadixCheckbox
							checked={settings.mediaAspectSquare}
							label="Square Thumbnails"
							name="mediaAspectSquare"
							onCheckedChange={(value) => {
								if (typeof value !== 'boolean') return;

								explorer.settingsStore.mediaAspectSquare = value;
							}}
						/>
					)}
				</div>
			</div>

			{settings.layoutMode === 'media' && (
				<div>
					<Subheading>Media View Context</Subheading>
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

			<div>
				<Subheading>Double click action</Subheading>
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
		</div>
	);
};

const doubleClickActions = z.union([
	z.literal('openFile').describe('Open File'),
	z.literal('quickPreview').describe('Quick Preview')
]);

const mediaViewContextActions = z.union([
	z.literal('withDescendants').describe('Current Directory With Descendants'),
	z.literal('withoutDescendants').describe('Current Directory')
]);
