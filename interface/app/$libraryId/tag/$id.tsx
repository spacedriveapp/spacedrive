import { getIcon, iconNames } from '@sd/assets/util';
import { useLibraryQuery } from '@sd/client';
import { LocationIdParamsSchema } from '~/app/route-schemas';
import { useZodRouteParams } from '~/hooks';
import Explorer from '../Explorer';
import { ExplorerContext } from '../Explorer/Context';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import { EmptyNotice } from '../Explorer/View';
import { TopBarPortal } from '../TopBar/Portal';

export const Component = () => {
	const { id: tagId } = useZodRouteParams(LocationIdParamsSchema);

	const explorerData = useLibraryQuery([
		'search.objects',
		{
			filter: {
				tags: [tagId]
			}
		}
	]);

	const tag = useLibraryQuery(['tags.get', tagId], { suspense: true });

	return (
		<ExplorerContext.Provider
			value={{
				parent: tag.data
					? {
							type: 'Tag',
							tag: tag.data
					  }
					: undefined
			}}
		>
			<TopBarPortal right={<DefaultTopBarOptions />} />
			<Explorer
				items={explorerData.data?.items || null}
				emptyNotice={
					<EmptyNotice
						icon={<img className="h-32 w-32" src={getIcon(iconNames.Tags)} />}
						message="No items assigned to this tag."
					/>
				}
			/>
		</ExplorerContext.Provider>
	);
};
