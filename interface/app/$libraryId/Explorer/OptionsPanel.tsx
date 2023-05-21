import { RadixCheckbox, Select, SelectOption, Slider, tw } from '@sd/ui';
import {
	ExplorerDirection,
	ExplorerOrderByKeys,
	getExplorerConfigStore,
	getExplorerStore,
	useExplorerConfigStore,
	useExplorerStore
} from '~/hooks';

const Heading = tw.div`text-ink-dull text-xs font-semibold`;
const Subheading = tw.div`text-ink-dull mb-1 text-xs font-medium`;

const sortOptions: Record<ExplorerOrderByKeys, string> = {
	none: 'None',
	name: 'Name',
	sizeInBytes: 'Size',
	dateCreated: 'Date created',
	dateModified: 'Date modified',
	dateIndexed: 'Date indexed',
	object: 'Object'
};

export default () => {
	const explorerStore = useExplorerStore();
	const explorerConfig = useExplorerConfigStore();

	return (
		<div className="p-4">
			<Subheading>Item size</Subheading>
			{explorerStore.layoutMode === 'media' ? (
				<Slider
					defaultValue={[10 - explorerStore.mediaColumns]}
					min={0}
					max={6}
					step={2}
					onValueChange={([val]) => {
						if (val !== undefined) {
							getExplorerStore().mediaColumns = 10 - val;
						}
					}}
				/>
			) : (
				<Slider
					onValueChange={(value) => {
						getExplorerStore().gridItemSize = value[0] || 100;
					}}
					defaultValue={[explorerStore.gridItemSize]}
					max={200}
					step={10}
					min={60}
				/>
			)}

			<div className="my-2 mt-4 grid grid-cols-2 gap-2">
				<div className="flex flex-col">
					<Subheading>Sort by</Subheading>
					<Select
						value={explorerStore.orderBy}
						size="sm"
						className="w-full"
						onChange={(value) =>
							(getExplorerStore().orderBy = value as ExplorerOrderByKeys)
						}
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
						onChange={(value) =>
							(getExplorerStore().orderByDirection = value as ExplorerDirection)
						}
					>
						<SelectOption value="asc">Asc</SelectOption>
						<SelectOption value="desc">Desc</SelectOption>
					</Select>
				</div>
			</div>
			<div className="flex w-full flex-col space-y-3 pt-2">
				{explorerStore.layoutMode === 'media' ? (
					<RadixCheckbox
						checked={explorerStore.mediaAspectSquare}
						label="Show square thumbnails"
						name="mediaAspectSquare"
						onCheckedChange={(value) => {
							if (typeof value === 'boolean') {
								getExplorerStore().mediaAspectSquare = value;
							}
						}}
					/>
				) : (
					<RadixCheckbox
						checked={explorerStore.showBytesInGridView}
						label="Show Object size"
						name="showBytesInGridView"
						onCheckedChange={(value) => {
							if (typeof value === 'boolean') {
								getExplorerStore().showBytesInGridView = value;
							}
						}}
					/>
				)}
				<div>
					<Subheading>Double click action</Subheading>
					<Select
						className="w-full"
						value={explorerConfig.openOnDoubleClick ? 'openFile' : 'quickPreview'}
						onChange={(value) => {
							getExplorerConfigStore().openOnDoubleClick = value === 'openFile';
						}}
					>
						<SelectOption value="openFile">Open File</SelectOption>
						<SelectOption value="quickPreview">Quick Preview</SelectOption>
					</Select>
				</div>
			</div>
		</div>
	);
};
