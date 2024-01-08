import Grid from '../Grid';
import { GridViewItem } from './Item';

export const GridView = () => {
	return (
		<Grid>
			{({ item, selected, cut }) => (
				<GridViewItem data={item} selected={selected} cut={cut} />
			)}
		</Grid>
	);
};
