import { Button, Input, Tabs } from '@sd/ui';
import { useState } from 'react';
import { DotsThree, Eye, EyeSlash } from 'phosphor-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { DefaultProps } from '../primitive/types';
import { KeyList } from './KeyList';
import { KeyMounter } from './KeyMounter';
import { useLibraryQuery, useLibraryMutation } from '@sd/client';
import { KeyDropdown } from './Key';
import { save } from '@tauri-apps/api/dialog';


export type KeyManagerProps = DefaultProps;

export function KeyManager(props: KeyManagerProps) {
	const hasMasterPw = useLibraryQuery(['keys.hasMasterPassword']);
	const setMasterPasswordMutation = useLibraryMutation('keys.setMasterPassword');
	const unmountAll = useLibraryMutation('keys.unmountAll');
	const clearMasterPassword = useLibraryMutation('keys.clearMasterPassword');
	const backupKeystore = useLibraryMutation('keys.backupKeystore');

	const [showMasterPassword, setShowMasterPassword] = useState(false);
	const [showSecretKey, setShowSecretKey] = useState(false);
	const [masterPassword, setMasterPassword] = useState('');
	const [secretKey, setSecretKey] = useState('');

	if(!hasMasterPw?.data) {	
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
							placeholder='Master Password'
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
							placeholder='Secret Key'
						/>
					<Button
						onClick={() => setShowSecretKey(!showSecretKey)}
						size="icon"
						className="border-none absolute right-[5px] top-[5px]"
					>
						<SKCurrentEyeIcon className="w-4 h-4" />
					</Button>
				</div>
	
				<Button className="w-full" variant="accent" onClick={() => {
					if(masterPassword !== "" && secretKey !== "") {
						setMasterPassword('');
						setSecretKey('');
						setMasterPasswordMutation.mutate({password: masterPassword, secret_key: secretKey}, {
							onError: () => {
								alert('Incorrect information provided.');
							}
						});
					} 
				}
				}>
					Unlock
				</Button>
			</div>
		);
	} else {
		return (
			<div>
				<Tabs.Root defaultValue="mount">
					<div className="flex flex-col">
						<Tabs.List>
							<Tabs.Trigger className="text-sm font-medium" value="mount">
								Mount
							</Tabs.Trigger>
							<Tabs.Trigger className="text-sm font-medium" value="keys">
								Keys
							</Tabs.Trigger>
							<div className="flex-grow" />
							<KeyDropdown trigger={
								<Button size="icon">
									<DotsThree className="w-4 h-4 text-ink-faint" />
								</Button> }>
								<DropdownMenu.DropdownMenuItem className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80" onClick={(e) => {
									unmountAll.mutate(null);
									clearMasterPassword.mutate(null);
								}}>Lock</DropdownMenu.DropdownMenuItem>
								<DropdownMenu.DropdownMenuItem className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80" onClick={(e) => {
									// not platform-safe, probably will break on web but `platform` doesn't have a save dialog option
									save()?.then((result) => {
										if (result)
											backupKeystore.mutate(result as string);
									});
								}}>Backup Keys</DropdownMenu.DropdownMenuItem>
								<DropdownMenu.DropdownMenuItem className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80" onClick={(e) => {
									
								}}>Restore Keys</DropdownMenu.DropdownMenuItem>
								<DropdownMenu.DropdownMenuItem className="!cursor-default select-none text-menu-ink focus:outline-none py-0.5 active:opacity-80" onClick={(e) => {
									
								}}>Change master password</DropdownMenu.DropdownMenuItem>
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
