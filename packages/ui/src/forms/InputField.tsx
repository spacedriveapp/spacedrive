import { zxcvbn, zxcvbnOptions } from '@zxcvbn-ts/core';
import zxcvbnCommonPackage from '@zxcvbn-ts/language-common';
import zxcvbnEnPackage from '@zxcvbn-ts/language-en';
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
	const [strength, setStrength] = useState<{ label: string; score: number }>();
	const updateStrength = useDebouncedCallback(
		() => setStrength(props.password ? getPasswordStrength(props.password) : undefined),
		100
	);

	// TODO: Remove duplicate in @sd/client
	function getPasswordStrength(password: string): { label: string; score: number } {
		const ratings = ['Poor', 'Weak', 'Good', 'Strong', 'Perfect'];

		zxcvbnOptions.setOptions({
			dictionary: {
				...zxcvbnCommonPackage.dictionary,
				...zxcvbnEnPackage.dictionary
			},
			graphs: zxcvbnCommonPackage.adjacencyGraphs,
			translations: zxcvbnEnPackage.translations
		});

		const result = zxcvbn(password);
		return { label: ratings[result.score]!, score: result.score };
	}

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
