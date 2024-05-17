import clsx from 'clsx';
import { ChangeEvent, ChangeEventHandler, forwardRef, memo } from 'react';
import { Input, toast } from '@sd/ui';
import i18n from '~/app/I18n';
import { useLocale, useOperatingSystem } from '~/hooks';
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
				message: value ? i18n.t('invalid_extension') : i18n.t('value_required')
			};
		}
		case 'Name': {
			// https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#:~:text=The following reserved characters
			const regex = os === 'windows' ? /^[^<>:"/\\|?*\u0000-\u0031]+$/ : /^[^/\0]+$/;
			return {
				value: regex.test(value),
				message: value ? i18n.t('invalid_name') : i18n.t('value_required')
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
				message: value ? i18n.t('invalid_path') : i18n.t('value_required')
			};
		}
		case 'Advanced': {
			const regex = os === 'windows' ? /^[^<>:"\u0000-\u0031]+$/ : /^[^\0]+/;
			return {
				value: regex.test(value),
				message: value ? i18n.t('invalid_glob') : i18n.t('value_required')
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
		const { t } = useLocale();

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
						placeholder={t('file_directory_name')}
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
						aria-label={t('add_file_extension_rule')}
						placeholder={t('file_extension_description')}
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
						placeholder={t('glob_description')}
						{...props}
					/>
				);
			default:
				throw new Error('Valid kind is required');
		}
	})
);
