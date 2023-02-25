import { Database, DotsSixVertical, Pencil, Trash } from 'phosphor-react';
import { LibraryConfigWrapped } from '@sd/client';
import { Button, ButtonLink, Card, Tooltip, dialogManager, tw } from '@sd/ui';
import DeleteDialog from './DeleteDialog';

const Pill = tw.span`px-1.5 ml-2 py-[2px] rounded text-xs font-medium bg-accent`;

export default (props: { library: LibraryConfigWrapped; current: boolean }) => {
	return (
		<Card>
			<DotsSixVertical weight="bold" className="mt-[15px] mr-3 opacity-30" />
			<div className="my-0.5 flex-1">
				<h3 className="font-semibold">
					{props.library.config.name}
					{props.current && <Pill>Current</Pill>}
				</h3>
				<p className="text-ink-dull mt-0.5 text-xs">{props.library.uuid}</p>
			</div>
			<div className="flex flex-row items-center space-x-2">
				<Button className="!p-1.5" variant="gray">
					<Tooltip label="TODO">
						<Database className="h-4 w-4" />
					</Tooltip>
				</Button>
				<ButtonLink className="!p-1.5" to="../../library/general" variant="gray">
					<Tooltip label="Edit Library">
						<Pencil className="h-4 w-4" />
					</Tooltip>
				</ButtonLink>
				<Button
					className="!p-1.5"
					variant="gray"
					onClick={() => {
						dialogManager.create((dp) => <DeleteDialog {...dp} libraryUuid={props.library.uuid} />);
					}}
				>
					<Tooltip label="Delete Library">
						<Trash className="h-4 w-4" />
					</Tooltip>
				</Button>
			</div>
		</Card>
	);
};
