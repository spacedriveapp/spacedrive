import { zodResolver } from '@hookform/resolvers/zod';
import { animated, useTransition } from '@react-spring/web';
import { VariantProps, cva } from 'class-variance-authority';
import { Warning } from 'phosphor-react';
import { ComponentProps } from 'react';
import {
	FieldErrors,
	FieldValues,
	FormProvider,
	UseFormHandleSubmit,
	UseFormProps,
	UseFormReturn,
	get,
	useForm,
	useFormContext
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
					e.preventDefault();
					return onSubmit?.(e);
				}}
				{...props}
			>
				{/**
				 * <fieldset> passes the form's 'disabled' state to all of its elements,
				 * allowing us to handle disabled style variants with just css.
				 * <fieldset> has a default `min-width: min-content`, which causes it to behave weirdly,
				 * so we override it.
				 */}
				<fieldset disabled={disabled || form.formState.isSubmitting} className="min-w-0">
					{children}
				</fieldset>
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

export const errorStyles = cva(
	'flex justify-center gap-2 break-all rounded border border-red-500/40 bg-red-800/40 px-3 py-2 text-white',
	{
		variants: {
			variant: {
				none: '',
				default: 'w-full text-xs',
				large: 'text-left text-xs font-semibold'
			}
		},
		defaultVariants: {
			variant: 'default'
		}
	}
);

export interface ErrorMessageProps extends VariantProps<typeof errorStyles> {
	name: string;
	className: string;
}

export const ErrorMessage = ({ name, variant, className }: ErrorMessageProps) => {
	const methods = useFormContext();
	const error = get(methods.formState.errors, name) as FieldErrors | undefined;
	const transitions = useTransition(error, {
		from: { opacity: 0 },
		enter: { opacity: 1 },
		leave: { opacity: 0 },
		clamp: true,
		config: { mass: 0.4, tension: 200, friction: 10, bounce: 0 },
		exitBeforeEnter: true
	});

	return (
		<>
			{transitions((styles, error) => {
				const message = error?.message;
				return typeof message === 'string' ? (
					<animated.div style={styles} className={errorStyles({ variant, className })}>
						<Warning className="h-4 w-4" />
						<p>{message}</p>
					</animated.div>
				) : null;
			})}
		</>
	);
};

export { z } from 'zod';
