import {
	Archive,
	Check,
	Copy,
	FolderDotted,
	Gear,
	IconContext,
	Image
} from '@phosphor-icons/react';
import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useLibraryMutation, type Location } from '@sd/client';
import {
	Button,
	Input,
	Popover,
	PopoverContainer,
	PopoverDivider,
	PopoverSection,
	toast,
	TOAST_TIMEOUT,
	Tooltip,
	tw,
	usePopover
} from '@sd/ui';
import { useLocale, useOperatingSystem } from '~/hooks';

import TopBarButton from '../TopBar/TopBarButton';

const OptionButton = tw(TopBarButton)`w-full gap-1 !px-1.5 !py-1`;

export default function LocationOptions({ location, path }: { location: Location; path: string }) {
	const navigate = useNavigate();

	const { t } = useLocale();
	const os = useOperatingSystem();

	const [copied, setCopied] = useState(false);

	const scanLocationSubPath = useLibraryMutation('locations.subPathRescan');
	const regenThumbs = useLibraryMutation('jobs.generateThumbsForLocation');

	const archiveLocation = () => alert('Not implemented');

	let currentPath = path ? location.path + path : location.path;

	currentPath = currentPath?.endsWith('/')
		? currentPath.substring(0, currentPath.length - 1)
		: currentPath;

	const osPath = os === 'windows' ? currentPath?.replace(/\//g, '\\') : currentPath;

	return (
		<div className="opacity-30 group-hover:opacity-70">
			<IconContext.Provider value={{ size: 20, className: 'r-1 h-4 w-4 opacity-60' }}>
				<Popover
					popover={usePopover()}
					trigger={
						<Button className="!p-[5px]" variant="subtle">
							<Ellipsis className="size-3" />
						</Button>
					}
				>
					<PopoverContainer>
						<PopoverSection>
							<Input
								readOnly
								className="mb-2"
								value={osPath ?? ''}
								right={
									<Tooltip
										label={copied ? t('copied') : t('copy_path_to_clipboard')}
										className="flex"
									>
										<Button
											size="icon"
											variant="outline"
											onClick={() => {
												if (!currentPath) return;

												navigator.clipboard.writeText(currentPath);

												toast.info({
													title: t('path_copied_to_clipboard_title'),
													body: t(
														'path_copied_to_clipboard_description',
														{ location: location.name }
													)
												});

												setCopied(true);
												setTimeout(() => setCopied(false), TOAST_TIMEOUT);
											}}
										>
											{copied ? (
												<Check size={16} className="text-green-400" />
											) : (
												<Copy size={16} className="opacity-70" />
											)}
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
								{t('configure_location')}
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
								{t('reindex')}
							</OptionButton>
							<OptionButton
								onClick={() => regenThumbs.mutate({ id: location.id, path })}
							>
								<Image />
								{t('regenerate_thumbs')}
							</OptionButton>
						</PopoverSection>
						<PopoverDivider />
						<PopoverSection>
							<OptionButton onClick={archiveLocation}>
								<Archive />
								{t('archive')}
							</OptionButton>
						</PopoverSection>
					</PopoverContainer>
				</Popover>
			</IconContext.Provider>
		</div>
	);
}
