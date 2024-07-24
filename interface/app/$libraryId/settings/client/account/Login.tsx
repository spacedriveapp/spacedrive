import { useZodForm } from '@sd/client';
import { Button, Form, Input, z } from '@sd/ui';

import { Controller } from 'react-hook-form';


const LoginSchema = z.object({
	email: z.string().email(),
	password: z.string().min(6),
})

const Login = () => {
	const form = useZodForm(
		{
			schema: LoginSchema,
			defaultValues: {
				email: '',
				password: '',
			}
		})
		return (
			<Form
			onSubmit={form.handleSubmit((data) => {
				// handle login submission
				console.log(data);
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

export default Login;