// import { ArrowsClockwise, Clipboard, Eye, EyeSlash } from '@phosphor-icons/react';
// import { useState } from 'react';
// import {
// 	Algorithm,
// 	HASHING_ALGOS,
// 	HashingAlgoSlug,
// 	hashingAlgoSlugSchema,
// 	useLibraryMutation
// } from '@sd/client';
// import { Button, Dialog, Input, Select, SelectOption, UseDialogProps, useDialog } from '@sd/ui';
// import { useZodForm, z } from '@sd/ui/src/forms';
// import { PasswordMeter, showAlertDialog } from '~/components';
// import { generatePassword } from '~/util';

// const schema = z.object({
// 	masterPassword: z.string(),
// 	masterPassword2: z.string(),
// 	encryptionAlgo: z.string(),
// 	hashingAlgo: hashingAlgoSlugSchema
// });

// export default (props: UseDialogProps) => {
// 	const changeMasterPassword = useLibraryMutation('keys.changeMasterPassword', {
// 		onSuccess: () => {
// 			showAlertDialog({
// 				title: 'Success',
// 				value: 'Your master password was changed successfully'
// 			});
// 		},
// 		onError: () => {
// 			// this should never really happen
// 			showAlertDialog({
// 				title: 'Master Password Change Error',
// 				value: 'There was an error while changing your master password.'
// 			});
// 		}
// 	});

// 	const [show, setShow] = useState({
// 		masterPassword: false,
// 		masterPassword2: false
// 	});

// 	const MP1CurrentEyeIcon = show.masterPassword ? EyeSlash : Eye;
// 	const MP2CurrentEyeIcon = show.masterPassword2 ? EyeSlash : Eye;

// 	const form = useZodForm({
// 		schema,
// 		defaultValues: {
// 			encryptionAlgo: 'XChaCha20Poly1305',
// 			hashingAlgo: 'Argon2id-s',
// 			masterPassword: '',
// 			masterPassword2: ''
// 		}
// 	});

// 	const onSubmit = form.handleSubmit((data) => {
// 		if (data.masterPassword !== data.masterPassword2) {
// 			showAlertDialog({
// 				title: 'Error',
// 				value: 'Passwords are not the same, please try again.'
// 			});
// 		} else {
// 			const hashing_algorithm = HASHING_ALGOS[data.hashingAlgo];

// 			return changeMasterPassword.mutateAsync({
// 				algorithm: data.encryptionAlgo as Algorithm,
// 				hashing_algorithm,
// 				password: data.masterPassword
// 			});
// 		}
// 	});

// 	return (
// 		<Dialog
// 			form={form}
// 			onSubmit={onSubmit}
// 			dialog={useDialog(props)}
// 			title="Change Master Password"
// 			description="Select a new master password for your key manager."
// 			ctaDanger={true}
// 			ctaLabel="Change"
// 		>
// 			<Input
// 				placeholder="New password"
// 				type={show.masterPassword ? 'text' : 'password'}
// 				className="mb-2 mt-3"
// 				{...form.register('masterPassword', { required: true })}
// 				right={
// 					<div className="flex">
// 						<Button
// 							onClick={() => {
// 								const password = generatePassword(32);
// 								form.setValue('masterPassword', password);
// 								form.setValue('masterPassword2', password);
// 								setShow((old) => ({
// 									...old,
// 									masterPassword: true,
// 									masterPassword2: true
// 								}));
// 							}}
// 							size="icon"
// 							type="button"
// 						>
// 							<ArrowsClockwise className="h-4 w-4" />
// 						</Button>
// 						<Button
// 							type="button"
// 							onClick={() => {
// 								navigator.clipboard.writeText(
// 									form.watch('masterPassword') as string
// 								);
// 							}}
// 							size="icon"
// 						>
// 							<Clipboard className="h-4 w-4" />
// 						</Button>
// 						<Button
// 							onClick={() =>
// 								setShow((old) => ({ ...old, masterPassword: !old.masterPassword }))
// 							}
// 							size="icon"
// 							type="button"
// 						>
// 							<MP1CurrentEyeIcon className="h-4 w-4" />
// 						</Button>
// 					</div>
// 				}
// 			/>

// 			<Input
// 				placeholder="New password (again)"
// 				type={show.masterPassword2 ? 'text' : 'password'}
// 				className="mb-2"
// 				{...form.register('masterPassword2', { required: true })}
// 				right={
// 					<Button
// 						onClick={() =>
// 							setShow((old) => ({ ...old, masterPassword2: !old.masterPassword2 }))
// 						}
// 						size="icon"
// 						type="button"
// 					>
// 						<MP2CurrentEyeIcon className="h-4 w-4" />
// 					</Button>
// 				}
// 			/>

// 			<PasswordMeter password={form.watch('masterPassword')} />

// 			<div className="mb-3 mt-4 grid w-full grid-cols-2 gap-4">
// 				<div className="flex flex-col">
// 					<span className="text-xs font-bold">Encryption</span>
// 					<Select
// 						className="mt-2"
// 						value={form.watch('encryptionAlgo')}
// 						onChange={(e) => form.setValue('encryptionAlgo', e)}
// 					>
// 						<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
// 						<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
// 					</Select>
// 				</div>
// 				<div className="flex flex-col">
// 					<span className="text-xs font-bold">Hashing</span>
// 					<Select
// 						className="mt-2"
// 						value={form.watch('hashingAlgo')}
// 						onChange={(e) => form.setValue('hashingAlgo', e as HashingAlgoSlug)}
// 					>
// 						<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
// 						<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
// 						<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
// 						<SelectOption value="BalloonBlake3-s">
// 							BLAKE3-Balloon (standard)
// 						</SelectOption>
// 						<SelectOption value="BalloonBlake3-h">
// 							BLAKE3-Balloon (hardened)
// 						</SelectOption>
// 						<SelectOption value="BalloonBlake3-p">
// 							BLAKE3-Balloon (paranoid)
// 						</SelectOption>
// 					</Select>
// 				</div>
// 			</div>
// 		</Dialog>
// 	);
// };
