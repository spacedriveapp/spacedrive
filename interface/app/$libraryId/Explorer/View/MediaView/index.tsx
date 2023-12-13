import { useExplorerContext } from '../../Context';
import Grid from '../Grid';
import { MediaViewItem } from './Item';

export const MediaView = () => {
	const explorerSettings = useExplorerContext().useSettingsSnapshot();

	return (
		<Grid>
			{({ item, selected, cut }) => (
				<MediaViewItem
					data={item}
					selected={selected}
					cut={cut}
					cover={explorerSettings.mediaAspectSquare}
				/>
			)}
		</Grid>
	);
};
