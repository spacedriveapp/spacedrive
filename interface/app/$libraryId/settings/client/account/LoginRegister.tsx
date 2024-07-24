import { zodResolver } from '@hookform/resolvers/zod';
import { useZodForm } from '@sd/client';
import { Button, Card, Divider, Form, Input, Tooltip, z } from '@sd/ui';
import { motion } from 'framer-motion';
import { useState } from 'react';

import { GoogleLogo, Icon } from '@phosphor-icons/react';
import { Apple, Github } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import { Controller, useForm } from 'react-hook-form';

const Tabs = ['Login', 'Register'] as const;
const LoginSchema = z.object({
	email: z.string().email(),
	password: z.string().min(6),
})
const RegisterSchema = z.object({
	email: z.string().email(),
	password: z.string().min(6),
	confirmPassword: z.string().min(6),
}).refine(data => data.password === data.confirmPassword, {
	message: 'Passwords do not match',
	path: ['confirmPassword']
})
type RegisterData = z.infer<typeof RegisterSchema>

type SocialLogin = {
	name: "Github" | "Google" | "Apple";
	icon: Icon;
}

const SocialLogins: SocialLogin[] = [
	{name: 'Github', icon: Github},
	{name: 'Google', icon: GoogleLogo},
	{name: 'Apple', icon: Apple},
]

const LoginRegister = () => {

	const [activeTab, setActiveTab] = useState<'Login' | 'Register'>('Login');

	const socialLoginHandlers = (name: SocialLogin['name']) => {
		return {
			'Github': () => {
				console.log('Github login');
			},
			'Google': () => {
				console.log('Google login');
			},
			'Apple': () => {
				console.log('Apple login');
			}
		}[name]();
	}

	return (
		<Card className="relative flex w-full max-w-[320px] flex-col items-center justify-center !p-0">
			<div className='flex w-full'>
				{Tabs.map((text) => (
					<div key={text} onClick={() => {
						setActiveTab(text)
				}} className={clsx("relative flex-1 border-b border-app-line p-2.5 text-center",
						text === 'Login' ? 'rounded-tl-md' : 'rounded-tr-md',
					)}>
						<p className={clsx('relative z-10 text-sm transition-colors duration-200',
							text === activeTab ? 'font-medium text-ink' : 'text-ink-faint'
						)}>{text}</p>
						{text === activeTab && (
						<motion.div
						animate={{
							borderRadius: text === 'Login' ? '0.3rem 0 0 0' : '0 0.3rem 0 0',
						}}
						layoutId='tab' className={clsx("absolute inset-x-0 top-0 z-0 size-full bg-app-line/60"
						)} />
						)}
					</div>
				))}
			</div>
		<div className='flex w-full flex-col justify-center gap-1.5 p-5'>
				{activeTab === 'Login' ? <Login/> : <Register/>}
			<div className='my-2 flex w-full items-center gap-3'>
			<Divider/>
			<p className='text-xs text-ink-faint'>OR</p>
			<Divider/>
			</div>
			<div className='flex justify-center gap-3'>
					{SocialLogins.map((social) => (
						<Tooltip key={social.name} label={social.name} position='bottom'>
						<div onClick={() => socialLoginHandlers(social.name)} key={social.name} className='rounded-full border border-app-line bg-app-input p-3'>
							<social.icon style={{
								fill: 'white'
							}} weight='bold' className='size-4'/>
						</div>
						</Tooltip>
					))}
			</div>
		</div>
		</Card>
	)
}

const Register = () => {

		// useZodForm seems to be out-dated or needs
		//fixing as it does not support the schema using zod.refine
		const form = useForm<RegisterData>(
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

export default LoginRegister;
