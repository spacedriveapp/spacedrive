import { Button, CategoryHeading, Input, Select, SelectOption } from '@sd/ui';
import clsx from 'clsx';
import { Eject, EjectSimple, Plus } from 'phosphor-react';
import { useState } from 'react';

import { Toggle } from '../primitive';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';
import { Key } from './Key';

export type KeyListProps = DefaultProps;

export function KeyList(props: KeyListProps) {
	return (
		<div className="flex flex-col h-full max-h-[360px]">
			<div className="p-3 custom-scroll overlay-scroll">
				<div className="">
					{/* <CategoryHeading>Mounted keys</CategoryHeading> */}
					<div className="space-y-1.5">
						<Key
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
						<Key index={3} data={{ id: 'b02303d68d05a562', name: 'Key 6' }} />
					</div>
				</div>
			</div>
			<div className="flex w-full p-2 bg-gray-600 border-t border-gray-500 rounded-b-md">
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
