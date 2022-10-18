import { Button, Input, Select, SelectOption, Tabs } from '@sd/ui';
import clsx from 'clsx';
import { Eject, EjectSimple, Plus } from 'phosphor-react';
import { useState } from 'react';

import { Toggle } from '../primitive';
import { DefaultProps } from '../primitive/types';
import { Tooltip } from '../tooltip/Tooltip';
import { Key } from './Key';
import { KeyList } from './KeyList';
import { KeyMounter } from './KeyMounter';

export type KeyManagerProps = DefaultProps;

export function KeyManager(props: KeyManagerProps) {
	return (
		<div>
			<Tabs.Root defaultValue="mount">
				<Tabs.List>
					<Tabs.Trigger className="text-sm font-medium text-gray-300" value="mount">
						Mount
					</Tabs.Trigger>
					<Tabs.Trigger className="text-sm font-medium text-gray-300" value="keys">
						Keys
					</Tabs.Trigger>
				</Tabs.List>
				<Tabs.Content value="keys">
					<KeyList />
				</Tabs.Content>
				<Tabs.Content value="mount">
					<KeyMounter />
				</Tabs.Content>
			</Tabs.Root>
		</div>
	);
}
