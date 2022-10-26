import { useLibraryQuery  } from '@sd/client';
import { Button } from '@sd/ui';

import { DefaultProps } from '../primitive/types';
import { Key } from './Key';

export type KeyListProps = DefaultProps;

const ListKeys = () => {
	const keys = useLibraryQuery(['keys.list']);
	const mounted_uuids = useLibraryQuery(['keys.listMounted']);

	return (
		<>
		{keys.data?.map((key, index) => {
			const active = !!keys.data?.find((t) => t.id === key.id);

			return (
				<Key index={index} data={{
					id: key.uuid,
					name: `Key ${index + 1}`,
					mounted: mounted_uuids.data?.includes(key.uuid)
				}} />
			)
		})}
		</>
	)
};

export function KeyList(props: KeyListProps) {
	return (
		<div className="flex flex-col h-full max-h-[360px]">
			<div className="p-3 custom-scroll overlay-scroll">
				<div className="">
					{/* <CategoryHeading>Mounted keys</CategoryHeading> */}
					<div className="space-y-1.5">
						<ListKeys></ListKeys>
						{/* <Key
							index={0}
							data={{
								id: 'af5570f5a1810b7a',
								name: 'OBS Recordings',
								mounted: true,

								nodes: ['node1', 'node2'],
								stats: { objectCount: 235, containerCount: 2 }
							}}
						/>
						<Key
							index={1}
							data={{
								id: 'af5570f5a1810b7a',
								name: 'Unknown Key',
								locked: true,
								mounted: true,
								stats: { objectCount: 45 }
							}}
						/>
						<Key index={2} data={{ id: '7324695a52da67b1', name: 'Spacedrive Company' }} />
						<Key index={3} data={{ id: 'b02303d68d05a562', name: 'Key 4' }} />
						<Key index={3} data={{ id: 'b02303d68d05a562', name: 'Key 5' }} />
						<Key index={3} data={{ id: 'b02303d68d05a562', name: 'Key 6' }} /> */}
					</div>
				</div>
			</div>
			<div className="flex w-full p-2 border-t border-app-line rounded-b-md">
				<Button size="sm" variant="gray">
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
