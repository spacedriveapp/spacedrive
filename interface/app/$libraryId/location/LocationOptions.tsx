import { Button, Popover, PopoverContainer, PopoverSection, Input, PopoverDivider, tw } from "@sd/ui";
import { Paperclip, Gear, FolderDotted, Archive, Image, Icon, IconContext, Copy } from "phosphor-react";
import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { Location, useLibraryMutation } from '@sd/client'

import TopBarButton from "../TopBar/TopBarButton";

const OptionButton = tw(TopBarButton)`w-full gap-1 !px-1.5 !py-1`

export default function LocationOptions({ location, path }: { location: Location, path: string }) {

	const _scanLocation = useLibraryMutation('locations.fullRescan');
	const scanLocation = () => _scanLocation.mutate(location.id);

	const _regenThumbs = useLibraryMutation('jobs.generateThumbsForLocation');
	const regenThumbs = () => _regenThumbs.mutate({ id: location.id, path });

	const archiveLocation = () => alert("Not implemented");

	let currentPath = path ? location.path + path : location.path;

	currentPath = currentPath.endsWith("/") ? currentPath.substring(0, currentPath.length - 1) : currentPath;


	return (
		<div className='opacity-30 group-hover:opacity-70'>
			<IconContext.Provider value={{ size: 20, className: "r-1 h-4 w-4 opacity-60" }}>

				<Popover trigger={<Button className="!p-[5px]" variant="subtle">
					<Ellipsis className="h-3 w-3" />
				</Button>}>
					<PopoverContainer>
						<PopoverSection>
							<Input autoFocus className='mb-2' value={currentPath} right={
								<Button
									size="icon"
									variant="outline"
									className='opacity-70'
								>
									<Copy className="!pointer-events-none h-4 w-4" />
								</Button>
							} />
							<OptionButton><Gear />Configure Location</OptionButton>
						</PopoverSection>
						<PopoverDivider />
						<PopoverSection>
							<OptionButton onClick={scanLocation}><FolderDotted />Re-index</OptionButton>
							<OptionButton onClick={regenThumbs}><Image />Regenerate Thumbs</OptionButton>
						</PopoverSection>
						<PopoverDivider />
						<PopoverSection>
							<OptionButton onClick={archiveLocation}><Archive />Archive</OptionButton>
						</PopoverSection>
					</PopoverContainer>
				</Popover>
			</IconContext.Provider>
		</div>
	)
}
