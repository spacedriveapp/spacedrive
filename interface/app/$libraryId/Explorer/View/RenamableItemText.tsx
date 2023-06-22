/* eslint-disable no-case-declarations */
import clsx from 'clsx';
import { ExplorerItem, getItemFilePath, getItemLocation } from '@sd/client';
import { useExplorerStore } from '~/hooks';
import { RenameLocationTextBox, RenamePathTextBox } from '../File/RenameTextBox';

export default function RenamableItemText(props: {
	item: ExplorerItem;
	selected: boolean;
	disabled?: boolean;
	allowHighlight?: boolean;
}) {
	const { item, selected, disabled, allowHighlight } = props;
	const explorerStore = useExplorerStore();

	const sharedProps = {
		className: clsx(
			'text-center font-medium text-ink',
			selected && allowHighlight !== false && 'bg-accent text-white dark:text-ink'
		),
		style: { maxHeight: explorerStore.gridItemSize / 3 },
		activeClassName: '!text-ink',
		disabled: !selected || disabled
	};

	switch (item.type) {
		case 'Path':
		case 'Object':
			const filePathData = getItemFilePath(item);
			if (!filePathData) break;
			return (
				<RenamePathTextBox
					itemId={filePathData.id}
					text={filePathData.name}
					extension={filePathData.extension}
					isDir={filePathData.is_dir || false}
					locationId={filePathData.location_id}
					{...sharedProps}
				/>
			);
		case 'Location':
			const locationData = getItemLocation(item);
			if (!locationData) break;
			return (
				<RenameLocationTextBox
					locationId={locationData.id}
					itemId={locationData.id}
					text={locationData.name}
					{...sharedProps}
				/>
			);
	}
	return <div />;
}
