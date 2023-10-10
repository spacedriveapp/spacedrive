import clsx from 'clsx';
import { ChangeEvent, ChangeEventHandler, forwardRef, memo } from 'react';
import { Input, toast } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { openDirectoryPickerDialog } from '../openDirectoryPickerDialog';

export type InputKinds = 'Name' | 'Extension' | 'Path' | 'Advanced';

interface Props {
	kind: InputKinds;
	className?: string;
	onChange?: ChangeEventHandler<HTMLInputElement> | undefined;
	onBlur?: ChangeEventHandler<HTMLInputElement> | undefined;
}

export const validateInput = (
	type: InputKinds,
	value: string,
	os?: string,
	isWeb?: boolean
): { value: boolean; message: string } | undefined => {
	// TODO: The os checks here shouldn't be for which os the UI is running, but for which os the node is running
	switch (type) {
		case 'Extension': {
			const regex = os === 'windows' ? /^\.[^<>:"/\\|?*\u0000-\u0031]+$/ : /^\.[^/\0\s]+$/;
			return {
				value: regex.test(value),
				message: value ? 'Invalid extension' : 'Value required'
			};
		}
		case 'Name': {
			// https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#:~:text=The following reserved characters
			const regex = os === 'windows' ? /^[^<>:"/\\|?*\u0000-\u0031]+$/ : /^[^/\0]+$/;
			return {
				value: regex.test(value),
				message: value ? 'Invalid name' : 'Value required'
			};
		}
		case 'Path': {
			const regex = isWeb
				? null // Non web plataforms use the native file picker, so there is no need to validate
				: os === 'windows'
				? /^[^<>:"/|?*\u0000-\u0031]+$/
				: /^[^\0]+$/;
			return {
				value: regex?.test(value) || false,
				message: value ? 'Invalid path' : 'Value required'
			};
		}
		case 'Advanced': {
			const regex = os === 'windows' ? /^[^<>:"\u0000-\u0031]+$/ : /^[^\0]+/;
			return {
				value: regex.test(value),
				message: value ? 'Invalid glob' : 'Value required'
			};
		}
		default:
			return undefined;
	}
};

export const RuleInput = memo(
	forwardRef<HTMLInputElement, Props>((props, ref) => {
		const os = useOperatingSystem(true);
		const platform = usePlatform();
		const isWeb = platform.platform === 'web';

		switch (props.kind) {
			case 'Name':
				return (
					<Input
						ref={ref}
						size="md"
						onBlur={(event) => {
							if (event.target.value) {
								props.onBlur?.(event);
							}
						}}
						// TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
						placeholder="File/Directory name"
						{...props}
					/>
				);
			case 'Extension':
				return (
					<Input
						ref={ref}
						size="md"
						onBlur={(event) => {
							if (event.target.value) {
								props.onBlur?.(event);
							}
						}}
						aria-label="Add a file extension to the current rule"
						placeholder="File extension (e.g., .mp4, .jpg, .txt)"
						{...props}
					/>
				);
			case 'Path':
				return (
					<Input
						ref={ref}
						size="md"
						onBlur={(event) => {
							if (event.target.value) {
								props.onBlur?.(event);
							}
						}}
						readOnly={!isWeb}
						className={clsx(props.className, isWeb || 'cursor-pointer')}
						placeholder={
							'Path (e.g., ' +
							// TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
							(os === 'windows'
								? 'C:\\Users\\john\\Downloads'
								: os === 'macOS'
								? '/Users/clara/Pictures'
								: '/home/emily/Documents') +
							')'
						}
						onClick={async () => {
							try {
								const path = await openDirectoryPickerDialog(platform);
								const event = {
									target: {
										value: path
									}
								} as ChangeEvent<HTMLInputElement>;
								if (path) {
									props.onChange?.(event);
								}
							} catch (error) {
								toast.error(String(error));
							}
						}}
						{...props}
					/>
				);
			case 'Advanced':
				return (
					<Input
						ref={ref}
						size="md"
						onBlur={(event) => {
							if (event.target.value) {
								props.onBlur?.(event);
							}
						}}
						placeholder="Glob (e.g., **/.git)"
						{...props}
					/>
				);
			default:
				throw new Error('Valid kind is required');
		}
	})
);
