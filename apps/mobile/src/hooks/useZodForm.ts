import { zodResolver } from '@hookform/resolvers/zod';
import { UseFormProps, useForm } from 'react-hook-form';
import { z } from 'zod';

interface UseZodFormProps<S extends z.ZodSchema>
	extends Exclude<UseFormProps<z.infer<S>>, 'resolver'> {
	schema?: S;
}

export const useZodForm = <S extends z.ZodSchema = z.ZodObject<Record<string, never>>>(
	props?: UseZodFormProps<S>
) => {
	const { schema, ...formProps } = props ?? {};

	return useForm({
		...formProps,
		resolver: zodResolver(schema || z.object({}))
	});
};

export { z } from 'zod';
