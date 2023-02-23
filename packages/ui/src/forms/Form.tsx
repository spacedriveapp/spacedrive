import { zodResolver } from '@hookform/resolvers/zod';
import { ComponentProps } from 'react';
import {
	FieldValues,
	FormProvider,
	UseFormHandleSubmit,
	UseFormProps,
	UseFormReturn,
	useForm
} from 'react-hook-form';
import { z } from 'zod';

export interface FormProps<T extends FieldValues> extends Omit<ComponentProps<'form'>, 'onSubmit'> {
	form: UseFormReturn<T>;
	onSubmit: ReturnType<UseFormHandleSubmit<T>>;
}

export const Form = <T extends FieldValues>({
	form,
	onSubmit,
	children,
	...props
}: FormProps<T>) => {
	const { className, ...otherProps } = props;
	return (
		<FormProvider {...form}>
			<form
				onSubmit={(e) => {
					e.stopPropagation();
					return onSubmit(e);
				}}
				{...otherProps}
			>
				{/* <fieldset> passes the form's 'disabled' state to all of its elements,
            allowing us to handle disabled style variants with just css */}
				<fieldset className={className} disabled={form.formState.isSubmitting}>
					{children}
				</fieldset>
			</form>
		</FormProvider>
	);
};

interface UseZodFormProps<S extends z.ZodSchema>
	extends Exclude<UseFormProps<z.infer<S>>, 'resolver'> {
	schema: S;
}

export const useZodForm = <S extends z.ZodSchema>({ schema, ...formProps }: UseZodFormProps<S>) =>
	useForm({
		...formProps,
		resolver: zodResolver(schema)
	});

export { z } from 'zod';
