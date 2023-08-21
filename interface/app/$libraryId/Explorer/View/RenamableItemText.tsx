/* eslint-disable no-case-declarations */
import clsx from 'clsx';
import { type ExplorerItem } from '@sd/client';
import { RenameLocationTextBox, RenamePathTextBox } from '../FilePath/RenameTextBox';

export default function RenamableItemText(props: {
	item: ExplorerItem;
	selected: boolean;
	disabled?: boolean;
	allowHighlight?: boolean;
	style?: React.CSSProperties;
}) {
	const { item, selected, disabled, allowHighlight, style } = props;

	const sharedProps = {
		className: clsx(
			'text-center font-medium text-ink',
			selected && allowHighlight !== false && 'bg-accent text-white dark:text-ink'
		),
		style: style,
		activeClassName: '!text-ink',
		disabled: !selected || disabled
	};

	if (item.type === 'Location') {
		const locationData = item.item;
		return (
			<RenameLocationTextBox
				locationId={locationData.id}
				itemId={locationData.id}
				text={locationData.name}
				{...sharedProps}
			/>
		);
	} else {
		const filePathData =
			item.type === 'Path' || item.type === 'NonIndexedPath'
				? item.item
				: item.type === 'Object'
				? item.item.file_paths[0]
				: null;

		if (filePathData) {
			return (
				<RenamePathTextBox
					itemId={'id' in filePathData ? filePathData.id : null}
					text={filePathData.name}
					extension={filePathData.extension}
					isDir={filePathData.is_dir || false}
					locationId={'location_id' in filePathData ? filePathData.location_id : null}
					{...sharedProps}
				/>
			);
		}
	}

	return <div />;
}
