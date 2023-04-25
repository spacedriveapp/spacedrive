import { PropsWithChildren, ReactNode, useId } from 'react';
import { useFormContext } from 'react-hook-form';
import { ErrorMessage } from './Form';

export interface UseFormFieldProps extends PropsWithChildren {
	name: string;
	label?: string;
	className?: string;
}

export const useFormField = <P extends UseFormFieldProps>(props: P) => {
	const { name, label, className, ...otherProps } = props;
	const { formState, getFieldState } = useFormContext();
	const state = getFieldState(props.name, formState);
	const id = useId();

	return {
		formFieldProps: { id, name, label, className, error: state.error?.message },
		childProps: { ...otherProps, id, name }
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
				<label htmlFor={props.id} className="mb-1 flex text-sm font-medium">
					{props.label}
				</label>
			)}
			{props.children}
			<ErrorMessage name={props.name} className="mt-1 w-full text-xs" />
		</div>
	);
};
