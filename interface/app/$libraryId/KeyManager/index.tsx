import { Gear, Lock } from 'phosphor-react';
import { useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, ButtonLink, Tabs } from '@sd/ui';
import KeyList from './List';
import KeyMounter from './Mounter';
import NotUnlocked from './NotUnlocked';

export function KeyManager() {
	const isUnlocked = useLibraryQuery(['keys.isUnlocked']);

	if (!isUnlocked?.data) return <NotUnlocked />;
	else return <Unlocked />;
}

const Unlocked = () => {
	const { library } = useLibraryContext();

	const unmountAll = useLibraryMutation('keys.unmountAll');
	const clearMasterPassword = useLibraryMutation('keys.clearMasterPassword');

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
						<div className="grow" />
						<Button
							size="icon"
							onClick={() => {
								unmountAll.mutate(null);
								clearMasterPassword.mutate(null);
							}}
							variant="subtle"
							className="text-ink-faint"
						>
							<Lock className="text-ink-faint h-4 w-4" />
						</Button>
						<ButtonLink
							to={`/${library.uuid}/settings/library/keys`}
							size="icon"
							variant="subtle"
							className="text-ink-faint"
						>
							<Gear className="text-ink-faint h-4 w-4" />
						</ButtonLink>
					</Tabs.List>
				</div>
				<Tabs.Content value="keys">
					<Keys />
				</Tabs.Content>
				<Tabs.Content value="mount">
					<KeyMounter />
				</Tabs.Content>
			</Tabs.Root>
		</div>
	);
};

const Keys = () => {
	const unmountAll = useLibraryMutation(['keys.unmountAll']);

	return (
		<div className="flex h-full max-h-[360px] flex-col">
			<div className="custom-scroll overlay-scroll p-3">
				<div className="">
					{/* <CategoryHeading>Mounted keys</CategoryHeading> */}
					<div className="space-y-1.5">
						<KeyList />
					</div>
				</div>
			</div>
			<div className="border-app-line flex w-full rounded-b-md border-t p-2">
				<Button
					size="sm"
					variant="gray"
					onClick={() => {
						unmountAll.mutate(null);
					}}
				>
					Unmount All
				</Button>
				<div className="grow" />
				<Button size="sm" variant="gray">
					Close
				</Button>
			</div>
		</div>
	);
};
