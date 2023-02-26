import { useBridgeQuery, useLibraryContext } from '@sd/client';
import { Button, dialogManager } from '@sd/ui';
import { Heading } from '../../Layout';
import CreateDialog from './CreateDialog';
import ListItem from './ListItem';

export default () => {
	const libraries = useBridgeQuery(['library.list']);

	const { library } = useLibraryContext();

	return (
		<>
			<Heading
				title="Libraries"
				description="The database contains all library data and file metadata."
				rightArea={
					<div className="flex-row space-x-2">
						<Button
							variant="accent"
							size="sm"
							onClick={() => {
								dialogManager.create((dp) => <CreateDialog {...dp} />);
							}}
						>
							Add Library
						</Button>
					</div>
				}
			/>

			<div className="space-y-2">
				{libraries.data
					?.sort((a, b) => {
						if (a.uuid === library.uuid) return -1;
						if (b.uuid === library.uuid) return 1;
						return 0;
					})
					.map((library) => (
						<ListItem
							current={library.uuid === library.uuid}
							key={library.uuid}
							library={library}
						/>
					))}
			</div>
		</>
	);
};
