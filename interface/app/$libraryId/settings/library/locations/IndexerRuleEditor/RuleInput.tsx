import clsx from 'clsx';
import { forwardRef, memo } from 'react';
import { ChangeEvent, ChangeEventHandler } from 'react';
import { Input } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { openDirectoryPickerDialog } from '../AddLocationDialog';

export type InputKinds = 'Name' | 'Extension' | 'Path' | 'Advanced';

interface Props {
	kind: InputKinds;
	className?: string;
	onChange?: ChangeEventHandler<HTMLInputElement> | undefined;
	onBlur?: ChangeEventHandler<HTMLInputElement> | undefined;
}

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
						pattern={os === 'windows' ? '[^<>:"/\\|?*\u0000-\u0031]*' : '[^/\0]+'}
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
						pattern="^\.[^\.\s]+$"
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
						pattern={
							isWeb
								? // Non web plataforms use the native file picker, so there is no need to validate
								  ''
								: // TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
								os === 'windows'
								? '[^<>:"/|?*\u0000-\u0031]*'
								: '[^\0]+'
						}
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
						onClick={() => {
							openDirectoryPickerDialog(platform)
								.then((path) => {
									const event = {
										target: {
											value: path
										}
									} as ChangeEvent<HTMLInputElement>;
									if (path) {
										props.onChange?.(event);
									}
								})
								.catch((error) =>
									showAlertDialog({
										title: 'Error',
										value: String(error)
									})
								);
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
						pattern={
							// TODO: The check here shouldn't be for which os the UI is running, but for which os the node is running
							os === 'windows' ? '[^<>:"\u0000-\u0031]*' : '[^\0]+'
						}
						placeholder="Glob (e.g., **/.git)"
						{...props}
					/>
				);
			default:
				throw new Error('Valid kind is required');
		}
	})
);
