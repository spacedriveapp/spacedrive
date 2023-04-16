import * as Tabs from '@radix-ui/react-tabs';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { Controller, useFormContext } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import {
	HASHING_ALGOS,
	LibraryConfigWrapped,
	hashingAlgoSlugSchema,
	useBridgeMutation,
	usePlausibleEvent
} from '@sd/client';
import {
	Dialog,
	DialogSteps,
	RadioGroup,
	SelectOption,
	UseDialogProps,
	forms,
	useDialog
} from '@sd/ui';
import { shareTelemetry } from '~/app/onboarding/privacy';

const { Input, z, useZodForm, PasswordInput, Select } = forms;

const nameSchema = z.object({ name: z.string().min(1) });
const passwordSchema = z
	.object({
		password: z.string(),
		password_validate: z.string(),
		algorithm: z.enum(['XChaCha20Poly1305', 'Aes256Gcm']),
		hashing_algorithm: hashingAlgoSlugSchema
	})
	.superRefine((data, ctx) => {
		if (data.password && data.password !== data.password_validate) {
			ctx.addIssue({
				code: 'custom',
				path: ['password_validate'],
				message: 'Passwords do not match'
			});
		}
	});
const privacySchema = z.object({ share_telemetry: shareTelemetry.schema });

const schema = nameSchema.and(passwordSchema).and(privacySchema);
type FormSchema = (typeof schema)['_output'];

const PasswordStep = () => {
	const form = useFormContext<FormSchema>();

	useEffect(() => {
		!form.getValues('password') && form.setFocus('password');
	}, []);

	return (
		<Tabs.Root defaultValue="password">
			<Tabs.List className="mb-4 inline-flex gap-0.5 rounded-md border border-app-line bg-app-selected/40 p-0.5 dark:bg-app">
				<Tabs.Trigger
					className="rounded px-2 py-1 text-sm transition hover:bg-app hover:shadow radix-state-active:bg-app radix-state-active:shadow dark:hover:bg-app-input dark:radix-state-active:bg-app-input"
					value="password"
				>
					Password
				</Tabs.Trigger>
				<Tabs.Trigger
					className="rounded px-2 py-1 text-sm transition hover:bg-app hover:shadow radix-state-active:bg-app radix-state-active:shadow dark:hover:bg-app-input dark:radix-state-active:bg-app-input"
					value="encryption"
				>
					Encryption
				</Tabs.Trigger>
			</Tabs.List>

			<Tabs.Content className="space-y-4 outline-none" value="password" tabIndex={-1}>
				<PasswordInput {...form.register('password')} label="Password" showStrength />
				<PasswordInput
					{...form.register('password_validate', {
						onBlur: () => form.trigger('password_validate')
					})}
					label="Confirm password"
				/>
			</Tabs.Content>
			<Tabs.Content className="space-y-4 outline-none" value="encryption" tabIndex={-1}>
				<Select control={form.control} name="algorithm" label="Algorithm" size="md">
					<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
					<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
				</Select>

				<Select control={form.control} name="hashing_algorithm" label="Hashing Algorithm" size="md">
					<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
					<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
					<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
					<SelectOption value="BalloonBlake3-s">BLAKE3-Balloon (standard)</SelectOption>
					<SelectOption value="BalloonBlake3-h">BLAKE3-Balloon (hardened)</SelectOption>
					<SelectOption value="BalloonBlake3-p">BLAKE3-Balloon (paranoid)</SelectOption>
				</Select>
			</Tabs.Content>
		</Tabs.Root>
	);
};

export default (props: UseDialogProps) => {
	const dialog = useDialog(props);
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const submitPlausibleEvent = usePlausibleEvent();

	const createLibrary = useBridgeMutation('library.create', {
		onSuccess: (library) => {
			queryClient.setQueryData(
				['library.list'],
				(libraries: LibraryConfigWrapped[] | undefined) => [...(libraries || []), library]
			);

			submitPlausibleEvent({
				event: {
					type: 'libraryCreate'
				}
			});

			navigate(`/${library.uuid}/overview`);
		},
		onError: (err) => console.log(err)
	});

	const form = useZodForm({
		schema,
		defaultValues: {
			password: '',
			password_validate: '',
			algorithm: 'XChaCha20Poly1305',
			hashing_algorithm: 'Argon2id-s',
			share_telemetry: 'share-telemetry'
		}
	});

	const password = form.watch('password');

	const steps: DialogSteps<FormSchema> = [
		{
			schema: nameSchema,
			title: 'Library name',
			description:
				'Choose a name for your new library, you can configure this and more settings from the library settings later on.',
			body: (
				<Input
					{...form.register('name')}
					label="Library name"
					placeholder={'e.g. "James\' Library"'}
					size="md"
				/>
			)
		},
		{
			schema: passwordSchema,
			title: 'Password',
			description: 'Choose a password for your new library.',
			skippable: !password,
			ctaLabel: !password ? 'Continue without' : undefined,
			body: <PasswordStep />
		},
		{
			schema: privacySchema,
			title: 'Privacy',
			description: 'Choose how you want to share your data with us.',
			body: (
				<Controller
					control={form.control}
					name="share_telemetry"
					render={({ field }) => (
						<RadioGroup.Root value={field.value} onValueChange={field.onChange}>
							{shareTelemetry.options.map(({ value, heading, description }, i) => (
								<RadioGroup.Item
									key={value}
									value={value}
									className="!border-app-selected !bg-app-button"
									radioClassName="dark:radix-state-unchecked:bg-app"
								>
									<h1 className="font-bold">{heading}</h1>
									<p className="text-sm text-ink-faint">{description}</p>
								</RadioGroup.Item>
							))}
						</RadioGroup.Root>
					)}
				/>
			)
		}
	];

	const onSubmit = form.handleSubmit(async (data) => {
		await createLibrary.mutateAsync({
			...data,
			algorithm: data.algorithm,
			hashing_algorithm: HASHING_ALGOS[data.hashing_algorithm],
			auth: {
				type: 'Password',
				value: data.password
			}
		});
	});

	return (
		<Dialog
			form={form}
			onSubmit={onSubmit}
			dialog={dialog}
			steps={steps}
			ctaLabel="Create library"
			submitDisabled={!form.formState.isValid}
			className="w-full"
		/>
	);
};
