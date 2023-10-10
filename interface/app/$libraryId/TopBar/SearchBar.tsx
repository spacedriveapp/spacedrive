import clsx from 'clsx';
import { useCallback, useEffect, useRef, useState, useTransition } from 'react';
import { useLocation, useNavigate, useResolvedPath } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import { useDebouncedCallback } from 'use-debounce';
import { Input, ModifierKeys, Shortcut } from '@sd/ui';
import { SearchParamsSchema } from '~/app/route-schemas';
import { getSearchStore, useOperatingSystem, useZodSearchParams } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

export default () => {
	const searchRef = useRef<HTMLInputElement>(null);

	const [searchParams, setSearchParams] = useZodSearchParams(SearchParamsSchema);
	const navigate = useNavigate();
	const location = useLocation();

	const os = useOperatingSystem(true);
	const keybind = keybindForOs(os);

	// Wrapping param updates in a transition allows us to track whether
	// updating the params triggers a Suspense somewhere else, providing a free
	// loading state!
	const [_isPending, startTransition] = useTransition();

	const searchPath = useResolvedPath('search');

	const [value, setValue] = useState(searchParams.search ?? '');

	const updateParams = useDebouncedCallback((value: string) => {
		startTransition(() =>
			setSearchParams((p) => ({ ...p, search: value }), {
				replace: true
			})
		);
	}, 300);

	const updateValue = useCallback(
		(value: string) => {
			setValue(value);
			if (searchPath.pathname === location.pathname) updateParams(value);
		},
		[searchPath.pathname, location.pathname, updateParams]
	);

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

	return (
		<Input
			ref={searchRef}
			placeholder="Search"
			className="mx-2 w-48 transition-all duration-200 focus-within:w-60"
			size="sm"
			onChange={(e) => updateValue(e.target.value)}
			onBlur={() => {
				getSearchStore().isFocused = false;
				if (value === '') {
					setSearchParams({}, { replace: true });
					navigate(-1);
				}
			}}
			onFocus={() => {
				getSearchStore().isFocused = true;
				if (searchPath.pathname !== location.pathname) {
					navigate({
						pathname: 'search',
						search: createSearchParams({ search: value }).toString()
					});
				}
			}}
			value={value}
			right={
				<>
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
					{/* This indicates whether the search is loading, a spinner could be put here */}
					{/* {_isPending && <div className="w-8 h-8 bg-red-500" />} */}
				</>
			}
		/>
	);
};
