import { PropsWithChildren, useId } from 'react';
import { useFormContext } from 'react-hook-form';

export interface UseFormFieldProps extends PropsWithChildren {
	name: string;
	// label: string;
}

export const useFormField = <P extends UseFormFieldProps>(props: P) => {
	const { name, ...otherProps } = props;
	const id = useId();

	return {
		formFieldProps: { id, name },
		childProps: { ...otherProps, id, name }
	};
};

interface FormFieldProps {
	name: string;
}

export const FormField = ({ name, children }: PropsWithChildren<FormFieldProps>) => {
	const ctx = useFormContext();
	const _ = ctx.getFieldState(name);

	return <>{children}</>;
};
