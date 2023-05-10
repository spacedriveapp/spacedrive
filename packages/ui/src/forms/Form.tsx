import { ErrorMessage as ErrorMessagePrimitive } from '@hookform/error-message';
import { zodResolver } from '@hookform/resolvers/zod';
import { VariantProps, cva } from 'class-variance-authority';
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
	disabled?: boolean;
	onSubmit?: ReturnType<UseFormHandleSubmit<T>>;
}

export const Form = <T extends FieldValues>({
	form,
	disabled,
	onSubmit,
	children,
	...props
}: FormProps<T>) => {
	return (
		<FormProvider {...form}>
			<form
				onSubmit={(e) => {
					e.stopPropagation();
					return onSubmit?.(e);
				}}
				{...props}
			>
				{/* <fieldset> passes the form's 'disabled' state to all of its elements,
            allowing us to handle disabled style variants with just css */}
				<fieldset disabled={disabled || form.formState.isSubmitting}>{children}</fieldset>
			</form>
		</FormProvider>
	);
};

interface UseZodFormProps<S extends z.ZodSchema>
	extends Exclude<UseFormProps<z.infer<S>>, 'resolver'> {
	schema?: S;
}

export const useZodForm = <S extends z.ZodSchema = z.ZodObject<Record<string, never>>>(
	props?: UseZodFormProps<S>
) => {
	const { schema, ...formProps } = props ?? {};

	return useForm<z.infer<S>>({
		...formProps,
		resolver: zodResolver(schema || z.object({}))
	});
};

export const errorStyles = cva('inline-block  whitespace-pre-wrap rounded border border-red-400/40 bg-red-400/40 text-white', {
	variants: {
		variant: {
			none: '',
			default: 'text-xs',
			large: 'w-full px-3 py-2 text-center text-sm font-semibold'
		}
	},
	defaultVariants: {
		variant: 'default'
	}
});

export interface ErrorMessageProps extends VariantProps<typeof errorStyles> {
	name: string;
	className: string;
}

export const ErrorMessage = ({ name, variant, className }: ErrorMessageProps) => (
	<ErrorMessagePrimitive as="span" name={name} className={errorStyles({ variant, className })} />
);

export { z } from 'zod';
