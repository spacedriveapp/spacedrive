import { useLibraryQuery, useLibraryMutation  } from '@sd/client';
import { Button, CategoryHeading } from '@sd/ui';

import { DefaultProps } from '../primitive/types';
import { Key } from './Key';
import type { Key as QueryKey } from '@sd/client';

export type KeyListProps = DefaultProps;

const ListKeys = () => {
	const keys = useLibraryQuery(['keys.list']);
	const mounted_uuids = useLibraryQuery(['keys.listMounted']);

	const mountedKeys: QueryKey[] = keys.data?.filter((key) => mounted_uuids.data?.includes(key.uuid)) ?? []
	const unmountedKeys: QueryKey[] = keys.data?.filter(key => !mounted_uuids.data?.includes(key.uuid)) ?? []

	if(keys.data?.length === 0) {
		return (
			<CategoryHeading>No keys available.</CategoryHeading>
		)
	}

	return (
		<>
		{[...mountedKeys, ...unmountedKeys]?.map((key, index) => {
			return (
				<Key index={index} data={{
					id: key.uuid,
					name: `Key ${index + 1}`,
					mounted: mountedKeys.includes(key)
				}} />
			)
		})}
		</>
	)
};

export function KeyList(props: KeyListProps) {
	const { mutate: unmountAll } = useLibraryMutation(['keys.unmountAll']);

	return (
		<div className="flex flex-col h-full max-h-[360px]">
			<div className="p-3 custom-scroll overlay-scroll">
				<div className="">
					{/* <CategoryHeading>Mounted keys</CategoryHeading> */}
					<div className="space-y-1.5">
						<ListKeys></ListKeys>
					</div>
				</div>
			</div>
			<div className="flex w-full p-2 border-t border-app-line rounded-b-md">
				<Button size="sm" variant="gray" onClick={() => {
					unmountAll(null);
				}}>
					Unmount All
				</Button>
				<div className="flex-grow" />
				<Button size="sm" variant="gray">
					Close
				</Button>
			</div>
		</div>
	);
}
