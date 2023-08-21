import { useLibraryContext } from '@sd/client';
import { ContextMenu, ModifierKeys } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { useKeybindFactory } from '~/hooks/useKeybindFactory';
import { NonEmptyArray } from '~/util';
import { Platform, usePlatform } from '~/util/Platform';

const lookup: Record<string, string> = {
	macOS: 'Finder',
	windows: 'Explorer'
};

export type RevealItems = NonEmptyArray<
	Parameters<NonNullable<Platform['revealItems']>>[1][number]
>;

export const RevealInNativeExplorerBase = (props: { items: RevealItems }) => {
	const os = useOperatingSystem();
	const keybind = useKeybindFactory();
	const { library } = useLibraryContext();
	const { revealItems } = usePlatform();
	if (!revealItems) return null;

	const osFileBrowserName = lookup[os] ?? 'file manager';

	return (
		<ContextMenu.Item
			label={`Reveal in ${osFileBrowserName}`}
			keybind={keybind([ModifierKeys.Control], ['Y'])}
			onClick={() => revealItems(library.uuid, props.items)}
		/>
	);
};
