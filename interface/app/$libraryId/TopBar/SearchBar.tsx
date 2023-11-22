import clsx from 'clsx';
import { useCallback, useEffect, useLayoutEffect, useRef, useState, useTransition } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { Input, ModifierKeys, Shortcut } from '@sd/ui';
import { SearchParamsSchema } from '~/app/route-schemas';
import { useOperatingSystem, useZodSearchParams } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import { useSearchStore } from '../Explorer/Search/store';

export default () => {
	const searchRef = useRef<HTMLInputElement>(null);

	const [searchParams, setSearchParams] = useZodSearchParams(SearchParamsSchema);

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
		console.log('blurHandler');
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

	const [localValue, setLocalValue] = useState(searchParams.search ?? '');

	useLayoutEffect(() => setLocalValue(searchParams.search ?? ''), [searchParams.search]);

	const updateValueDebounced = useDebouncedCallback((value: string) => {
		setSearchParams((p) => ({ ...p, search: value }), { replace: true });
	}, 300);

	function updateValue(value: string) {
		setLocalValue(value);
		updateValueDebounced(value);
	}

	function clearValue() {
		setSearchParams(
			(p) => {
				delete p.search;
				return { ...p };
			},
			{ replace: true }
		);
	}

	return (
		<Input
			ref={searchRef}
			placeholder="Search"
			className="mx-2 w-48 transition-all duration-200 focus-within:w-60"
			size="sm"
			value={localValue}
			onChange={(e) => updateValue(e.target.value)}
			onBlur={() => {
				if (localValue === '' && !searchStore.interactingWithSearchOptions) clearValue();
			}}
			onFocus={() => updateValueDebounced(localValue)}
			right={
				<div
					className={clsx(
						'pointer-events-none flex h-7 items-center space-x-1 opacity-70 group-focus-within:hidden'
					)}
				>
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
