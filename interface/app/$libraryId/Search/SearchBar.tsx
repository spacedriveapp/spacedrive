import { useCallback, useEffect, useRef, useState } from 'react';
import { Input, ModifierKeys, Shortcut } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import { useSearchContext } from '../Search';
import { useSearchStore } from '../Search/store';

export default () => {
	const search = useSearchContext();
	const searchRef = useRef<HTMLInputElement>(null);

	const searchStore = useSearchStore();

	const os = useOperatingSystem(true);
	const keybind = keybindForOs(os);

	const focusHandler = useCallback(
		(event: KeyboardEvent) => {
			if (
				event.key.toUpperCase() === 'F' &&
				event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			) {
				event.preventDefault();
				searchRef.current?.focus();
			}
		},
		[os]
	);

	const blurHandler = useCallback((event: KeyboardEvent) => {
		if (event.key === 'Escape' && document.activeElement === searchRef.current) {
			// Check if element is in focus, then remove it
			event.preventDefault();
			searchRef.current?.blur();
		}
	}, []);

	useEffect(() => {
		const input = searchRef.current;
		document.body.addEventListener('keydown', focusHandler);
		input?.addEventListener('keydown', blurHandler);
		return () => {
			document.body.removeEventListener('keydown', focusHandler);
			input?.removeEventListener('keydown', blurHandler);
		};
	}, [blurHandler, focusHandler]);

	const [value, setValue] = useState(search.search);

	function updateValue(value: string) {
		setValue(value);
		search.setSearch(value);
	}

	function clearValue() {
		setValue('');
		search.setSearch('');
	}

	return (
		<Input
			ref={searchRef}
			placeholder="Search"
			className="mx-2 w-48 transition-all duration-200 focus-within:w-60"
			size="sm"
			value={value}
			onChange={(e) => updateValue(e.target.value)}
			onBlur={() => {
				if (search.rawSearch === '' && !searchStore.interactingWithSearchOptions) {
					clearValue();
					search.setOpen(false);
				}
			}}
			onFocus={() => search.setOpen(true)}
			right={
				<div className="pointer-events-none flex h-7 items-center space-x-1 opacity-70 group-focus-within:hidden">
					{
						<Shortcut
							chars={keybind([ModifierKeys.Control], ['F'])}
							aria-label={`Press ${
								os === 'macOS' ? 'Command' : ModifierKeys.Control
							}-F to focus search bar`}
							className="border-none"
						/>
					}
				</div>
			}
		/>
	);
};
