import clsx from 'clsx';
import { useEffect, useState, useTransition } from 'react';
import { useRef } from 'react';
import { useLocation, useMatch, useMatches, useNavigate, useResolvedPath } from 'react-router';
import { useSearchParams } from 'react-router-dom';
import { useDebouncedCallback } from 'use-debounce';
import { Input, Shortcut } from '@sd/ui';
import { useLibraryContext } from '~/../packages/client/src';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

export const SEARCH_PARAM_KEY = 'search';

export default () => {
	const searchRef = useRef<HTMLInputElement>(null);

	const [searchParams, setSearchParams] = useSearchParams();
	const navigate = useNavigate();
	const location = useLocation();

	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	const { library } = useLibraryContext();

	useEffect(() => {
		const keyboardSearchFocus = (event: KeyboardEvent) => {
			if (!searchRef.current) return;

			if (event.key === 'f' && (event.metaKey || event.ctrlKey)) {
				event.preventDefault();
				searchRef.current?.focus();
			} else if (searchRef.current === document.activeElement && event.key === 'Escape') {
				setSearchParams((p) => (p.delete(SEARCH_PARAM_KEY), p));
				searchRef.current?.blur();
			}
		};

		document.addEventListener('keydown', keyboardSearchFocus);

		return () => {
			document.removeEventListener('keydown', keyboardSearchFocus);
		};
	}, [searchRef]);

	// Wrapping param updates in a transition allows us to track whether
	// updating the params triggers a Suspense somewhere else, providing a free
	// loading state!
	const [_isPending, startTransition] = useTransition();

	const searchPath = useResolvedPath('search');

	const [value, setValue] = useState(searchParams.get(SEARCH_PARAM_KEY) || '');

	const updateParams = useDebouncedCallback((value: string) => {
		startTransition(() =>
			setSearchParams((p) => (p.set(SEARCH_PARAM_KEY, value), p), {
				replace: true
			})
		);
	}, 300);

	useEffect(() => {
		updateParams(value);
	}, [value]);

	return (
		<Input
			ref={searchRef}
			placeholder="Search"
			className="w-52 transition-all duration-200 focus-within:w-60"
			size="sm"
			onChange={(e) => setValue(e.target.value)}
			onBlur={() => {
				if (value === '') {
					setSearchParams((p) => (p.delete(SEARCH_PARAM_KEY), p), { replace: true });
					navigate(-1);
				}
			}}
			onFocus={() => {
				if (searchPath.pathname !== location.pathname) {
					// Replace here so that navigate(-1) functions properly
					navigate(`search`, { replace: true });
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
							/>
						) : os === 'macOS' ? (
							<Shortcut
								chars="⌘F"
								aria-label={'Press Command-F to focus search bar'}
							/>
						) : (
							<Shortcut
								chars="CTRL+F"
								aria-label={'Press CTRL-F to focus search bar'}
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
