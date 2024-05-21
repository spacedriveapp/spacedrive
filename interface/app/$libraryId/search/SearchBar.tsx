import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import { useDebouncedCallback } from 'use-debounce';
import { SearchFilterArgs } from '@sd/client';
import { Input, ModifierKeys, Shortcut } from '@sd/ui';
import { useLocale, useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import { useSearchContext } from './context';
import { useSearchStore } from './store';
import { SearchTarget } from './useSearch';

interface Props {
	redirectToSearch?: boolean;
	defaultFilters?: SearchFilterArgs[];
	defaultTarget?: SearchTarget;
}

export default ({ redirectToSearch, defaultFilters, defaultTarget }: Props) => {
	const search = useSearchContext();
	const searchRef = useRef<HTMLInputElement>(null);
	const navigate = useNavigate();
	const searchStore = useSearchStore();
	const locationState: { focusSearch?: boolean } = useLocation().state;

	const os = useOperatingSystem(true);
	const keybind = keybindForOs(os);

	const focusHandler = useCallback(
		(event: KeyboardEvent) => {
			if (
				event.key.toUpperCase() === 'F' &&
				event.getModifierState(os === 'macOS' ? ModifierKeys.Meta : ModifierKeys.Control)
			) {
				searchRef.current?.focus();
			}

			const handler = () => searchRef.current?.focus();

			document.addEventListener('open_search', handler);
			return () => document.removeEventListener('open_search', handler);
		},
		[os]
	);

	const blurHandler = useCallback((event: KeyboardEvent) => {
		//condition prevents default search of webview
		if (event.key === 'f' && event.ctrlKey) {
			event.preventDefault();
		}
		if (event.key === 'Escape' && document.activeElement === searchRef.current) {
			event.preventDefault();
			// Check if element is in focus, then remove it
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

	const [value, setValue] = useState(search.rawSearch);

	useEffect(() => {
		if (search.rawSearch !== undefined) setValue(search.rawSearch);
	}, [search.rawSearch]);

	const updateDebounce = useDebouncedCallback((value: string) => {
		search.setSearch?.(value);
		if (redirectToSearch) {
			navigate(
				{
					pathname: '../search',
					search: createSearchParams({
						search: value
					}).toString()
				},
				{
					state: {
						focusSearch: true
					}
				}
			);
		}
	}, 300);

	function updateValue(value: string) {
		setValue(value);
		updateDebounce(value);
	}

	function clearValue() {
		search.setSearch?.(undefined);
		search.setFilters?.(undefined);
		search.setTarget?.(undefined);
	}

	const { t } = useLocale();

	return (
		<Input
			ref={searchRef}
			placeholder={t('search')}
			className="mx-2 w-48 transition-all duration-200 focus-within:w-60"
			size="sm"
			value={value}
			onChange={(e) => {
				updateValue(e.target.value);
			}}
			autoFocus={locationState?.focusSearch || false}
			onBlur={() => {
				if (search.rawSearch === '' && !searchStore.interactingWithSearchOptions) {
					clearValue();
					search.setSearchBarFocused(false);
				}
			}}
			onFocus={() => {
				search.setSearchBarFocused(true);
				search.setFilters?.((f) => {
					if (!f) return defaultFilters ?? [];
					else return f;
				});
				search.setTarget?.(search.target ?? defaultTarget);
			}}
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
