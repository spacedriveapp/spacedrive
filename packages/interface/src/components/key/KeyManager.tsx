import { Tabs } from '@sd/ui';

import { DefaultProps } from '../primitive/types';
import { KeyList } from './KeyList';
import { KeyMounter } from './KeyMounter';

export type KeyManagerProps = DefaultProps;

export function KeyManager(props: KeyManagerProps) {
	return (
		<div>
			<Tabs.Root defaultValue="mount">
				<Tabs.List>
					<Tabs.Trigger className="text-sm font-medium" value="mount">
						Mount
					</Tabs.Trigger>
					<Tabs.Trigger className="text-sm font-medium" value="keys">
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
