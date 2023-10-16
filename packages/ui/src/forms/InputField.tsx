import clsx from 'clsx';
import { forwardRef, useEffect, useState } from 'react';
import { useFormContext } from 'react-hook-form';
import { useDebouncedCallback } from 'use-debounce';

import * as Root from '../Input';
import { FormField, useFormField, UseFormFieldProps } from './FormField';

export interface InputFieldProps extends UseFormFieldProps, Root.InputProps {
	name: string;
}

export const InputField = forwardRef<HTMLInputElement, InputFieldProps>((props, ref) => {
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root.Input {...childProps} ref={ref} error={formFieldProps.error !== undefined} />
		</FormField>
	);
});

export interface PasswordInputProps extends UseFormFieldProps, Root.InputProps {
	name: string;
	showStrength?: boolean;
}

const PasswordStrengthMeter = (props: { password: string }) => {
	const [zxcvbn, setZxcvbn] = useState<typeof import('./zxcvbn') | undefined>();

	const [strength, setStrength] = useState<{ label: string; score: number }>();
	const updateStrength = useDebouncedCallback(() => {
		if (!zxcvbn) return;

		setStrength(props.password ? getPasswordStrength(props.password, zxcvbn) : undefined);
	}, 100);

	// TODO: Remove duplicate in @sd/client
	function getPasswordStrength(
		password: string,
		zxcvbn: typeof import('./zxcvbn')
	): { label: string; score: number } {
		const ratings = ['Poor', 'Weak', 'Good', 'Strong', 'Perfect'];

		zxcvbn.zxcvbnOptions.setOptions({
			dictionary: {
				...zxcvbn.languageCommon.dictionary,
				...zxcvbn.languageEn.dictionary
			},
			graphs: zxcvbn.languageCommon.adjacencyGraphs,
			translations: zxcvbn.languageEn.translations
		});

		const result = zxcvbn.zxcvbn(password);
		return { label: ratings[result.score]!, score: result.score };
	}

	useEffect(() => {
		let cancelled = false;

		import('./zxcvbn').then((zxcvbn) => {
			if (cancelled) return;
			setZxcvbn(zxcvbn);
		});

		return () => {
			cancelled = true;
		};
	}, []);

	useEffect(() => updateStrength(), [props.password, updateStrength]);

	return (
		<div className="flex grow items-center justify-end">
			{strength && (
				<span
					className={clsx(
						'mr-2 text-xs transition-[color]',
						strength.score === 0 && 'text-red-500',
						strength.score === 1 && 'text-red-500',
						strength.score === 2 && 'text-amber-400',
						strength.score === 3 && 'text-lime-500',
						strength.score === 4 && 'text-accent'
					)}
				>
					{strength.label}
				</span>
			)}

			<div className={clsx('h-[6px] w-1/4 rounded-full bg-app-selected')}>
				{strength && (
					<div
						style={{
							width: `${strength.score !== 0 ? strength.score * 25 : 12.5}%`
						}}
						className={clsx(
							'h-full rounded-full transition-[width]',
							strength.score === 0 && 'bg-red-500',
							strength.score === 1 && 'bg-red-500',
							strength.score === 2 && 'bg-amber-400',
							strength.score === 3 && 'bg-lime-500',
							strength.score === 4 && 'bg-accent'
						)}
					/>
				)}
			</div>
		</div>
	);
};

export const PasswordInputField = forwardRef<HTMLInputElement, PasswordInputProps>(
	({ showStrength, ...props }, ref) => {
		const { formFieldProps, childProps } = useFormField(props);
		const { watch } = useFormContext();

		return (
			<FormField
				{...formFieldProps}
				label={
					<>
						{formFieldProps.label}
						{showStrength && <PasswordStrengthMeter password={watch(props.name)} />}
					</>
				}
			>
				<Root.PasswordInput
					{...childProps}
					ref={ref}
					error={formFieldProps.error !== undefined}
				/>
			</FormField>
		);
	}
);
