import clsx from 'clsx';
import { forwardRef } from 'react';
import { useFormContext } from 'react-hook-form';
import { InputField, InputFieldProps, toast } from '@sd/ui';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { openDirectoryPickerDialog } from './openDirectoryPickerDialog';

export const LocationPathInputField = forwardRef<
	HTMLInputElement,
	Omit<InputFieldProps, 'onClick' | 'readOnly' | 'className'>
>((props, ref) => {
	const platform = usePlatform();
	const form = useFormContext();
	const { t } = useLocale();

	return (
		<InputField
			{...props}
			ref={ref}
			onClick={() =>
				openDirectoryPickerDialog(platform)
					.then(
						(path) =>
							path &&
							form.setValue(props.name, path, {
								shouldDirty: true
							})
					)
					.catch((error) => toast.error(t('error_message', { error: String(error) })))
			}
			readOnly={platform.platform !== 'web'}
			className={clsx('mb-3', platform.platform === 'web' || 'cursor-pointer')}
		/>
	);
});
