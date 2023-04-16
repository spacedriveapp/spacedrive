import { useQueryClient } from '@tanstack/react-query';
import { Controller } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import {
	HASHING_ALGOS,
	LibraryConfigWrapped,
	hashingAlgoSlugSchema,
	useBridgeMutation,
	usePlausibleEvent
} from '@sd/client';
import { Dialog, DialogSteps, RadioGroup, UseDialogProps, forms, useDialog } from '@sd/ui';
import { shareTelemetry } from '~/app/onboarding/privacy';

const { Input, z, useZodForm, PasswordInput } = forms;

const nameSchema = z.object({ name: z.string().min(1) });
const passwordSchema = z
	.object({
		password: z.string(),
		password_validate: z.string()
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

const schema = nameSchema
	.and(passwordSchema)
	.and(privacySchema)
	.and(
		z.object({
			algorithm: z.enum(['XChaCha20Poly1305', 'Aes256Gcm']),
			hashing_algorithm: hashingAlgoSlugSchema
		})
	);

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

	const steps: DialogSteps<(typeof schema)['_output']> = [
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
			body: (
				<>
					<PasswordInput {...form.register('password')} label="Password" autoFocus showStrength />
					<PasswordInput
						{...form.register('password_validate', {
							onBlur: () => form.trigger('password_validate')
						})}
						label="Confirm password"
					/>
				</>
			)
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
