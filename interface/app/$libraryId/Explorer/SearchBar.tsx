import clsx from 'clsx';
import { ComponentPropsWithRef, forwardRef, useEffect } from 'react';
import { useForm } from 'react-hook-form';
import { Input, Shortcut } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

interface SearchBarProps extends ComponentPropsWithRef<'input'> {
	setToggleSearch?: (value: boolean) => void;
	toggleSearch?: boolean;
}

export default forwardRef<HTMLInputElement, SearchBarProps>((props, forwardedRef) => {
	const {
		register,
		handleSubmit,
		reset,
		formState: { dirtyFields }
	} = useForm();

	const { ref, ...searchField } = register('searchField', {
		onBlur: () => {
			// if there's no text in the search bar, don't mark it as dirty so the key hint shows
			if (!dirtyFields.searchField) reset();
		}
	});

	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	useEffect(() => {
		//When the search bar is toggled, focus the search bar
		if (typeof forwardedRef !== 'function') {
			if (props.toggleSearch && forwardedRef?.current) {
				forwardedRef.current.focus();
			}
		}
		//Handling closing search bar when its been opened through the search icon
		const closeOnOutsideCLick = (event: MouseEvent) => {
			if (typeof forwardedRef !== 'function') {
				if (
					forwardedRef?.current === document.activeElement &&
					event.target !== forwardedRef.current
				) {
					props.setToggleSearch && props.setToggleSearch(false);
					forwardedRef.current?.blur();
					reset();
				}
			}
		};
		const keyboardSearchFocus = (event: KeyboardEvent) => {
			if (typeof forwardedRef !== 'function') {
				if ((event.key === 'f' && event.metaKey) || event.ctrlKey) {
					event.preventDefault();
					props.setToggleSearch && props.setToggleSearch(true);
					forwardedRef?.current?.focus();
				} else if (forwardedRef?.current === document.activeElement && event.key === 'Escape') {
					props.setToggleSearch && props.setToggleSearch(false);
					forwardedRef.current?.blur();
					reset();
					// this check is for the case when the search bar is opened through the search icon
				} else if (event.key === 'Enter' && forwardedRef?.current) {
					props.setToggleSearch && props.setToggleSearch(false);
					forwardedRef.current?.blur();
					reset();
				}
			}
		};
		document.addEventListener('mousedown', closeOnOutsideCLick);
		document.addEventListener('keydown', keyboardSearchFocus);
		return () => {
			document.removeEventListener('mousedown', closeOnOutsideCLick);
			document.removeEventListener('keydown', keyboardSearchFocus);
		};
	}, [props, forwardedRef, reset]);

	return (
		<form onSubmit={handleSubmit(() => null)} className="relative flex h-7">
			<Input
				ref={(el) => {
					ref(el);
					if (typeof forwardedRef === 'function') forwardedRef(el);
					else if (forwardedRef) forwardedRef.current = el;
				}}
				placeholder="Search"
				className={clsx('w-52 transition-all duration-200 focus-within:w-60', props.className)}
				size="sm"
				{...searchField}
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
