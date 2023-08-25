import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { Archive, Copy, FolderDotted, Gear, IconContext, Image } from 'phosphor-react';
import { useNavigate } from 'react-router';
import { type Location, useLibraryMutation } from '@sd/client';
import {
	Button,
	Input,
	Popover,
	PopoverContainer,
	PopoverDivider,
	PopoverSection,
	Tooltip,
	tw
} from '@sd/ui';
import TopBarButton from '../TopBar/TopBarButton';

const OptionButton = tw(TopBarButton)`w-full gap-1 !px-1.5 !py-1`;

export default function LocationOptions({ location, path }: { location: Location; path: string }) {
	const navigate = useNavigate();

	const scanLocationSubPath = useLibraryMutation('locations.subPathRescan');
	const regenThumbs = useLibraryMutation('jobs.generateThumbsForLocation');

	const archiveLocation = () => alert('Not implemented');

	let currentPath = path ? location.path + path : location.path;

	currentPath = currentPath?.endsWith('/')
		? currentPath.substring(0, currentPath.length - 1)
		: currentPath;

	return (
		<div className="opacity-30 group-hover:opacity-70">
			<IconContext.Provider value={{ size: 20, className: 'r-1 h-4 w-4 opacity-60' }}>
				<Popover
					trigger={
						<Button className="!p-[5px]" variant="subtle">
							<Ellipsis className="h-3 w-3" />
						</Button>
					}
				>
					<PopoverContainer>
						<PopoverSection>
							<Input
								autoFocus
								className="mb-2"
								value={currentPath ?? ''}
								right={
									<Tooltip label="Copy path to clipboard" className="flex">
										<Button
											size="icon"
											variant="outline"
											className="opacity-70"
											onClick={() =>
												currentPath &&
												navigator.clipboard.writeText(currentPath)
											}
										>
											<Copy className="!pointer-events-none h-4 w-4" />
										</Button>
									</Tooltip>
								}
							/>
							<OptionButton
								onClick={() =>
									navigate(`../settings/library/locations/${location.id}`)
								}
							>
								<Gear />
								Configure Location
							</OptionButton>
						</PopoverSection>
						<PopoverDivider />
						<PopoverSection>
							<OptionButton
								onClick={() =>
									scanLocationSubPath.mutate({
										location_id: location.id,
										sub_path: path ?? ''
									})
								}
							>
								<FolderDotted />
								Re-index
							</OptionButton>
							<OptionButton
								onClick={() => regenThumbs.mutate({ id: location.id, path })}
							>
								<Image />
								Regenerate Thumbs
							</OptionButton>
						</PopoverSection>
						<PopoverDivider />
						<PopoverSection>
							<OptionButton onClick={archiveLocation}>
								<Archive />
								Archive
							</OptionButton>
						</PopoverSection>
					</PopoverContainer>
				</Popover>
			</IconContext.Provider>
		</div>
	);
}
