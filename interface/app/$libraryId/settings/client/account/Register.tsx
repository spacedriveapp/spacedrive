import { zodResolver } from '@hookform/resolvers/zod';
import { Button, Form, Input, z } from '@sd/ui';

import { Controller, useForm } from 'react-hook-form';


const RegisterSchema = z.object({
	email: z.string().email(),
	password: z.string().min(6),
	confirmPassword: z.string().min(6),
}).refine(data => data.password === data.confirmPassword, {
	message: 'Passwords do not match',
	path: ['confirmPassword']
})
type RegisterType = z.infer<typeof RegisterSchema>

const Register = () => {

	// useZodForm seems to be out-dated or needs
	//fixing as it does not support the schema using zod.refine
	const form = useForm<RegisterType>(
	{
		resolver: zodResolver(RegisterSchema),
		defaultValues: {
			email: '',
			password: '',
			confirmPassword: '',
		}
	})
	return (
		<Form
		onSubmit={form.handleSubmit((data) => {
			// handle login submission
			return console.log(data);
			})}
		form={form}
		>
					<div className='flex flex-col gap-1.5'>
		<Controller
				control={form.control}
				name="email"
				render={({ field }) => (
					<Input
						{...field}
						placeholder="Email"
						error={Boolean(form.formState.errors.email?.message)}
						type="email"
						disabled={form.formState.isSubmitting}
					/>
				)}
			/>
			{form.formState.errors.email && (
				<p className="text-xs text-red-500">{form.formState.errors.email.message}</p>
			)}
			<Controller
				control={form.control}
				name="password"
				render={({ field }) => (
					<Input
						{...field}
						placeholder="Password"
						error={Boolean(form.formState.errors.password?.message)}
						type="password"
						className='w-full'
						disabled={form.formState.isSubmitting}
					/>
				)}
			/>
			{form.formState.errors.password && (
				<p className="text-xs text-red-500">{form.formState.errors.password.message}</p>
			)}
			<Controller
				control={form.control}
				name="confirmPassword"
				render={({ field }) => (
					<Input
						{...field}
						placeholder="Confirm Password"
						error={Boolean(form.formState.errors.confirmPassword?.message)}
						type="password"
						className='w-full'
						disabled={form.formState.isSubmitting}
					/>
				)}
				/>
					{form.formState.errors.confirmPassword && (
				<p className="text-xs text-red-500">{form.formState.errors.confirmPassword.message}</p>
			)}
				<Button
				type="submit"
				className='mx-auto mt-2 w-full'
				variant="accent"
				onClick={form.handleSubmit((data) => {
					console.log(data);
				})}
				disabled={form.formState.isSubmitting}
			>
				Submit
			</Button>
			</div>
		</Form>
	)
}

export default Register;
