import { PropsWithChildren, ReactNode, useId } from 'react';
import { useFormContext } from 'react-hook-form';

import { Label } from '../Input';
import { tw } from '../utils';
import { ErrorMessage } from './Form';

export const InfoText = tw.p`text-xs text-ink-faint`;

export interface UseFormFieldProps extends PropsWithChildren {
	name: string;
	label?: string;
	className?: string;
	formFieldClassName?: string;
}

export const useFormField = <P extends UseFormFieldProps>(props: P) => {
	const { name, label, className, formFieldClassName, ...otherProps } = props;
	const { formState, getFieldState } = useFormContext();
	const state = getFieldState(props.name, formState);
	const id = useId();

	return {
		formFieldProps: {
			id,
			name,
			label,
			error: state.error?.message,
			className: formFieldClassName
		},
		childProps: { ...otherProps, id, name, className }
	};
};

interface FormFieldProps extends Omit<UseFormFieldProps, 'label'> {
	id: string;
	name: string;
	label?: string | ReactNode;
}

export const FormField = (props: FormFieldProps) => {
	return (
		<div className={props.className}>
			{props.label && (
				<Label slug={props.id} className="mb-1 flex font-semibold">
					{props.label}
				</Label>
			)}
			{props.children}
			<ErrorMessage name={props.name} className="mt-1 w-full text-xs" />
		</div>
	);
};
