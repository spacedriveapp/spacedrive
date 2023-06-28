import { Database, Database_Light } from '@sd/assets/icons';
import { Pencil, Trash } from 'phosphor-react';
import { LibraryConfigWrapped } from '@sd/client';
import { Button, ButtonLink, Card, Tooltip, dialogManager, tw } from '@sd/ui';
import { useIsDark } from '~/hooks';
import DeleteDialog from './DeleteDialog';

interface Props {
	library: LibraryConfigWrapped;
	current: boolean;
}

export default (props: Props) => {
	const isDark = useIsDark();

	return (
		<Card className="items-center">
			{/* <DotsSixVertical weight="bold" className="mt-[15px] mr-3 opacity-30" /> */}
			<img
				className="mr-3"
				width={30}
				height={30}
				src={isDark ? Database : Database_Light}
				alt="Database icon"
			/>
			<div className="my-0.5 flex-1">
				<h3 className="font-semibold">
					{props.library.config.name}
					{props.current && (
						<span className="ml-2 rounded bg-accent px-1.5 py-[2px] text-xs font-medium text-white">
							Current
						</span>
					)}
				</h3>
				<p className="mt-0.5 text-xs text-ink-dull">{props.library.uuid}</p>
			</div>
			<div className="flex flex-row items-center space-x-2">
				{/* <Button className="!p-1.5" variant="gray">
				<Tooltip label="TODO">
					<Database className="h-4 w-4" />
				</Tooltip>
			</Button> */}
				<ButtonLink
					className="!p-1.5"
					to={`/${props.library.uuid}/settings/library/general`}
					variant="gray"
				>
					<Tooltip label="Edit Library">
						<Pencil className="h-4 w-4" />
					</Tooltip>
				</ButtonLink>
				<Button
					className="!p-1.5"
					variant="gray"
					onClick={() => {
						dialogManager.create((dp) => (
							<DeleteDialog {...dp} libraryUuid={props.library.uuid} />
						));
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
