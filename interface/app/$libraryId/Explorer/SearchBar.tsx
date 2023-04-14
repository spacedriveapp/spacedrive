import clsx from 'clsx';
import { ComponentPropsWithRef, forwardRef, useEffect } from 'react';
import { useState } from 'react';
import { Input, Shortcut } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

interface Props extends ComponentPropsWithRef<'input'> {
	formClassName?: string;
}

export default forwardRef<HTMLInputElement, Props>((props, forwardedRef) => {
	const [searchValue, setSearchValue] = useState('');
	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	useEffect(() => {
		const keyboardSearchFocus = (event: KeyboardEvent) => {
			if (typeof forwardedRef !== 'function') {
				if ((event.key === 'f' && event.metaKey) || event.ctrlKey) {
					event.preventDefault();
					forwardedRef?.current?.focus();
				} else if (forwardedRef?.current === document.activeElement && event.key === 'Escape') {
					forwardedRef.current?.blur();
					setSearchValue('');
				}
			}
		};
		document.addEventListener('keydown', keyboardSearchFocus);
		return () => {
			document.removeEventListener('keydown', keyboardSearchFocus);
		};
	}, [forwardedRef]);

	return (
		<form data-tauri-drag-region className={`relative flex h-7 ${props.formClassName}`}>
			<Input
				ref={(el) => {
					if (typeof forwardedRef === 'function') forwardedRef(el);
					else if (forwardedRef) forwardedRef.current = el;
				}}
				placeholder="Search"
				className={clsx('w-52 transition-all duration-200 focus-within:w-60', props.className)}
				size="sm"
				onChange={(e) => setSearchValue(e.target.value)}
				onBlur={() => setSearchValue('')}
				value={searchValue}
				right={
					<div
						className={clsx(
							'pointer-events-none flex h-7 items-center space-x-1 opacity-70 group-focus-within:hidden'
						)}
					>
						{platform === 'browser' ? (
							<Shortcut chars="⌘F" aria-label={'Press Command-F to focus search bar'} />
						) : os === 'macOS' ? (
							<Shortcut chars="⌘F" aria-label={'Press Command-F to focus search bar'} />
						) : (
							<Shortcut chars="CTRL+F" aria-label={'Press CTRL-F to focus search bar'} />
						)}
					</div>
				}
			/>
		</form>
	);
});
