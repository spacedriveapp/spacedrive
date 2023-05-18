import clsx from 'clsx';
import { useCallback, useRef, useState, useTransition } from 'react';
import { useLocation, useNavigate, useResolvedPath } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import { useKey, useKeys } from 'rooks';
import { useDebouncedCallback } from 'use-debounce';
import { z } from 'zod';
import { Input, Shortcut } from '@sd/ui';
import { useZodSearchParams } from '~/hooks';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { getSearchStore } from '~/hooks/useSearchStore';

export const SEARCH_PARAM_KEY = 'search';

export const SEARCH_PARAMS = z.object({
	search: z.string().default('')
});

export default () => {
	const searchRef = useRef<HTMLInputElement>(null);

	const [searchParams, setSearchParams] = useZodSearchParams(SEARCH_PARAMS);
	const navigate = useNavigate();
	const location = useLocation();

	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	// Wrapping param updates in a transition allows us to track whether
	// updating the params triggers a Suspense somewhere else, providing a free
	// loading state!
	const [_isPending, startTransition] = useTransition();

	const searchPath = useResolvedPath('search');

	const [value, setValue] = useState(searchParams.search);

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

	useKeys([os === 'macOS' ? 'Meta' : 'Ctrl', 'f'], () => searchRef.current?.focus());
	useKey('Escape', () => searchRef.current?.blur());

	return (
		<Input
			ref={searchRef}
			placeholder="Search"
			className="w-52 transition-all duration-200 focus-within:w-60"
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
						{platform === 'browser' ? (
							<Shortcut
								chars="⌘F"
								aria-label={'Press Command-F to focus search bar'}
								className="border-none"
							/>
						) : os === 'macOS' ? (
							<Shortcut
								chars="⌘F"
								aria-label={'Press Command-F to focus search bar'}
								className="border-none"
							/>
						) : (
							<Shortcut
								chars="CTRL+F"
								aria-label={'Press CTRL-F to focus search bar'}
								className="border-none"
							/>
						)}
					</div>
					{/* This indicates whether the search is loading, a spinner could be put here */}
					{/* {_isPending && <div className="h-8 w-8 bg-red-500" />} */}
				</>
			}
		/>
	);
};
