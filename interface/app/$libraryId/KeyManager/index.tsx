import { Gear, Lock, MagnifyingGlass, X } from 'phosphor-react';
import { useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, Tabs } from '@sd/ui';
import KeyList from './List';
import KeyMounter from './Mounter';
import NotSetup from './NotSetup';
import NotUnlocked from './NotUnlocked';

export function KeyManager() {
	const isUnlocked = useLibraryQuery(['keys.isUnlocked']);
	const isSetup = useLibraryQuery(['keys.isSetup']);

	if (!isSetup?.data) return <NotSetup />;
	if (!isUnlocked?.data) return <NotUnlocked />;
	else return <Unlocked />;
}

const Unlocked = () => {
	const { library } = useLibraryContext();
	const isUnlocked = useLibraryQuery(['keys.isUnlocked']);

	const unmountAll = useLibraryMutation('keys.unmountAll');
	const clearMasterPassword = useLibraryMutation('keys.clearMasterPassword');

	return (
		<div className="w-[350px]">
			<Tabs.Root defaultValue="keys">
				<div className="min-w-32 flex flex-col">
					<Tabs.List>
						{/* <Input placeholder="Search" /> */}
						{/* <Tabs.Trigger className="!rounded-md text-sm font-medium" value="mount">
							Mount
						</Tabs.Trigger>
						<Tabs.Trigger className="!rounded-md text-sm font-medium" value="keys">
							Keys
						</Tabs.Trigger> */}
						<Button size="icon" variant="subtle" className="text-ink-faint">
							<MagnifyingGlass className="h-4 w-4 text-ink-faint" />
						</Button>
						<div className="grow" />
						<Button
							size="icon"
							onClick={() => {
								unmountAll
									.mutateAsync(null)
									.then(() => clearMasterPassword.mutateAsync(null))
									.then(() => isUnlocked.refetch());
							}}
							variant="subtle"
							className="text-ink-faint"
						>
							<Lock className="h-4 w-4 text-ink-faint" />
						</Button>
						<Button size="icon" variant="subtle" className="text-ink-faint">
							<Gear className="h-4 w-4 text-ink-faint" />
						</Button>
						<Button size="icon" variant="subtle" className="text-ink-faint">
							<X className="h-4 w-4 text-ink-faint" />
						</Button>
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
	return (
		<div className="flex h-full max-h-[360px] flex-col">
			<div className="custom-scroll overlay-scroll p-3">
				<div className="">
					<div className="space-y-1.5">
						<KeyList />
					</div>
				</div>
			</div>
		</div>
	);
};
