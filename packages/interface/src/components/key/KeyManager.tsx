import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Input, Select, SelectOption, Tabs } from '@sd/ui';
import { Dialog } from '@sd/ui';
import { save } from '@tauri-apps/api/dialog';
import { DotsThree, Eye, EyeSlash } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';
import { Algorithm, HashingAlgorithm, Params } from '@sd/client';

import { DefaultProps } from '../primitive/types';
import { KeyDropdown } from './Key';
import { KeyList } from './KeyList';
import { KeyMounter } from './KeyMounter';

export type KeyManagerProps = DefaultProps;

export function KeyManager(props: KeyManagerProps) {
	const hasMasterPw = useLibraryQuery(['keys.hasMasterPassword']);
	const setMasterPasswordMutation = useLibraryMutation('keys.setMasterPassword');
	const unmountAll = useLibraryMutation('keys.unmountAll');
	const clearMasterPassword = useLibraryMutation('keys.clearMasterPassword');
	const backupKeystore = useLibraryMutation('keys.backupKeystore');
	const changeMasterPassword = useLibraryMutation('keys.changeMasterPassword');

	const [showMasterPassword, setShowMasterPassword] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);
	
	const [masterPassword, setMasterPassword] = useState('');
	const [secretKey, setSecretKey] = useState('');

	const [masterPasswordDialog, setMasterPasswordDialog] = useState(false);

	const [masterPasswordChange1, setMasterPasswordChange1] = useState('');
	const [masterPasswordChange2, setMasterPasswordChange2] = useState('');
	const [encryptionAlgo, setEncryptionAlgo] = useState('XChaCha20Poly1305');
	const [hashingAlgo, setHashingAlgo] = useState('Argon2id-s');

	if (!hasMasterPw?.data) {
		const MPCurrentEyeIcon = showMasterPassword ? EyeSlash : Eye;
		const SKCurrentEyeIcon = showSecretKey ? EyeSlash : Eye;

		return (
			<div className="p-2">
				<div className="relative flex flex-grow mb-2">
					<Input
						value={masterPassword}
						onChange={(e) => setMasterPassword(e.target.value)}
						autoFocus
						type={showMasterPassword ? 'text' : 'password'}
						className="flex-grow !py-0.5"
						placeholder="Master Password"
					/>
					<Button
						onClick={() => setShowMasterPassword(!showMasterPassword)}
						size="icon"
						className="border-none absolute right-[5px] top-[5px]"
					>
						<MPCurrentEyeIcon className="w-4 h-4" />
					</Button>
				</div>

				<div className="relative flex flex-grow mb-2">
					<Input
						value={secretKey}
						onChange={(e) => setSecretKey(e.target.value)}
						type={showSecretKey ? 'text' : 'password'}
						className="flex-grow !py-0.5"
						placeholder="Secret Key"
					/>
					<Button
						onClick={() => setShowSecretKey(!showSecretKey)}
						size="icon"
						className="border-none absolute right-[5px] top-[5px]"
					>
						<SKCurrentEyeIcon className="w-4 h-4" />
					</Button>
				</div>

				<Button
					className="w-full"
					variant="accent"
					onClick={() => {
						if (masterPassword !== '' && secretKey !== '') {
							setMasterPassword('');
							setSecretKey('');
							setMasterPasswordMutation.mutate(
								{ password: masterPassword, secret_key: secretKey },
								{
									onError: () => {
										alert('Incorrect information provided.');
									}
								}
							);
						}
					}}
				>
					Unlock
				</Button>
			</div>
		);
	} else {
		return (
			<div>
				<Tabs.Root defaultValue="mount">
					<div className="flex flex-col">
						<Dialog
							open={masterPasswordDialog}
							setOpen={setMasterPasswordDialog}
							title="Change Master Password"
							description="Select a new master password for your key manager."
							ctaAction={() => {
								if(masterPasswordChange1 !== "" && masterPasswordChange2 !== "") {
									if(masterPasswordChange1 !== masterPasswordChange2) {
										alert("Passwords are not the same.");
									} else {
										setMasterPasswordChange1('');
										setMasterPasswordChange2('');
									
										const algorithm = encryptionAlgo as Algorithm;
										let hashing_algorithm: HashingAlgorithm = { Argon2id: "Standard" };
						
										switch(hashingAlgo) {
											case "Argon2id-s":
												hashing_algorithm = { Argon2id: "Standard" as Params };
												break;
											case "Argon2id-h":
												hashing_algorithm = { Argon2id: "Hardened" as Params };
												break;
											case "Argon2id-p":
												hashing_algorithm = { Argon2id: "Paranoid" as Params };
												break;
										}
						
										changeMasterPassword.mutate({algorithm, hashing_algorithm, password: masterPasswordChange1 }, {
											onSuccess: (sk) => {
												alert("Your new secret key is: " + sk);
											}
										});
									}
								}
							}}
							ctaLabel="Change"
							trigger={
								<></>
							}
						>
							<Input
								className="flex-grow w-full mt-3"
								value={masterPasswordChange1}
								placeholder="Password"
								onChange={(e) => setMasterPasswordChange1(e.target.value)}
								required
								type={'password'}
							/>
							<Input
								className="flex-grow w-full mt-3"
								value={masterPasswordChange2}
								placeholder="Password (again)"
								onChange={(e) => setMasterPasswordChange2(e.target.value)}
								required
								type={'password'}
							/>

							<div className="grid w-full grid-cols-2 gap-4 mt-4 mb-3">
								<div className="flex flex-col">
									<span className="text-xs font-bold">Encryption</span>
									<Select className="mt-2" onChange={setEncryptionAlgo} value={encryptionAlgo}>
										<SelectOption value="XChaCha20Poly1305">XChaCha20-Poly1305</SelectOption>
										<SelectOption value="Aes256Gcm">AES-256-GCM</SelectOption>
									</Select>
								</div>
								<div className="flex flex-col">
									<span className="text-xs font-bold">Hashing</span>
									<Select className="mt-2" onChange={setHashingAlgo} value={hashingAlgo}>
										<SelectOption value="Argon2id-s">Argon2id (standard)</SelectOption>
										<SelectOption value="Argon2id-h">Argon2id (hardened)</SelectOption>
										<SelectOption value="Argon2id-p">Argon2id (paranoid)</SelectOption>
									</Select>
								</div>
							</div>
						</Dialog>
						<Tabs.List>
							<Tabs.Trigger className="text-sm font-medium" value="mount">
								Mount
							</Tabs.Trigger>
							<Tabs.Trigger className="text-sm font-medium" value="keys">
								Keys
							</Tabs.Trigger>
							<div className="flex-grow" />
							<KeyDropdown
								trigger={
									<Button size="icon">
										<DotsThree className="w-4 h-4 text-ink-faint" />
									</Button>
								}
							>
								<DropdownMenu.DropdownMenuItem
									className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
									onClick={(e) => {
										unmountAll.mutate(null);
										clearMasterPassword.mutate(null);
									}}
								>
									Lock
								</DropdownMenu.DropdownMenuItem>
								<DropdownMenu.DropdownMenuItem
									className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
									onClick={(e) => {
										// not platform-safe, probably will break on web but `platform` doesn't have a save dialog option
										save()?.then((result) => {
											if (result) backupKeystore.mutate(result as string);
										});
									}}
								>
									Backup Keys
								</DropdownMenu.DropdownMenuItem>
								<DropdownMenu.DropdownMenuItem
									className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
									onClick={(e) => {}}
								>
									Restore Keys
								</DropdownMenu.DropdownMenuItem>

								<DropdownMenu.DropdownMenuItem
									className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80"
									onClick={(e) => {
										setMasterPasswordDialog(true);
									}}
								>
									Change master password
								</DropdownMenu.DropdownMenuItem>

							</KeyDropdown>
						</Tabs.List>
					</div>

					<Tabs.Content value="keys">
						<KeyList />
					</Tabs.Content>
					<Tabs.Content value="mount">
						<KeyMounter />
					</Tabs.Content>
				</Tabs.Root>
			</div>
		);
	}
}
