import { PropsWithChildren, useId } from 'react';
import { useFormContext } from 'react-hook-form';

export interface UseFormFieldProps extends PropsWithChildren {
	name: string;
	label?: string;
	required?: boolean;
	className?: string;
}

export const useFormField = <P extends UseFormFieldProps>(props: P) => {
	const { name, label, required, className, ...otherProps } = props;
	const { formState, getFieldState } = useFormContext();
	const state = getFieldState(props.name, formState);
	const id = useId();

	return {
		formFieldProps: { id, name, label, required, className, error: state.error?.message },
		childProps: { ...otherProps, id, name }
	};
};

interface FormFieldProps extends UseFormFieldProps {
	id: string;
	error?: string;
}

export const FormField = (props: FormFieldProps) => {
	return (
		<div className={props.className}>
			{props.label && (
				<label htmlFor={props.id} className="mb-1 flex text-sm font-medium">
					{props.label}
					{props.required && <span className="ml-1 text-red-500">*</span>}
				</label>
			)}
			{props.children}
			{props.error && <span className="mt-1 text-xs text-red-500">{props.error}</span>}
		</div>
	);
};
