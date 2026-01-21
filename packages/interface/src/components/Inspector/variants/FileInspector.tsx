import {
	ArrowsClockwise,
	Calendar,
	ChatCircle,
	ClockCounterClockwise,
	Cube,
	DotsThree,
	FilmStrip,
	Fingerprint,
	HardDrive,
	Hash,
	Image,
	Info,
	MagnifyingGlass,
	MapPin,
	Microphone,
	Palette,
	Paperclip,
	PaperPlaneRight,
	Sparkle,
	Tag as TagIcon,
	TextAa,
	Trash,
	VideoCamera
} from '@phosphor-icons/react';
import {getIcon} from '@sd/assets/util';
import type {File} from '@sd/ts-client';
import {toast} from '@sd/ui';
import clsx from 'clsx';
import {useState} from 'react';
import {useJobsContext} from '../../../components/JobManager/hooks/JobsContext';
import {TagSelectorButton} from '../../../components/Tags';
import {usePlatform} from '../../../contexts/PlatformContext';
import {useServer} from '../../../contexts/ServerContext';
import { getContentKind } from "@sd/ts-client";
import {
	getDeviceIcon,
	useLibraryMutation,
	useNormalizedQuery
} from '../../../contexts/SpacedriveContext';
import {useContextMenu} from '../../../hooks/useContextMenu';
import {File as FileComponent} from '../../../routes/explorer/File';
import { formatBytes } from '../../../routes/explorer/utils';
import {Divider, InfoRow, Section, TabContent, Tabs, Tag} from '../Inspector';

interface FileInspectorProps {
	file: File;
}

export function FileInspector({file}: FileInspectorProps) {
	const [activeTab, setActiveTab] = useState('overview');
	const isDev = import.meta.env.DEV;

	const fileQuery = useNormalizedQuery<{file_id: string}, File>({
		wireMethod: 'query:files.by_id',
		input: {file_id: file?.id || ''},
		resourceType: 'file',
		resourceId: file?.id, // Filter batch events to only this file
		enabled: !!file?.id
	});

	const fileData = fileQuery.data || file;

	const tabs = [
		{id: 'overview', label: 'Overview', icon: Info},
		{id: 'sidecars', label: 'Sidecars', icon: Image},
		{id: 'instances', label: 'Instances', icon: MapPin},
		...(isDev
			? [{id: 'chat', label: 'Chat', icon: ChatCircle, badge: 3}]
			: []),
		...(isDev
			? [{id: 'activity', label: 'Activity', icon: ClockCounterClockwise}]
			: []),
		{id: 'details', label: 'More', icon: DotsThree}
	];

	return (
		<>
			{/* Tabs */}
			<Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

			{/* Tab Content */}
			<div className="mt-2.5 flex flex-1 flex-col overflow-hidden">
				<TabContent id="overview" activeTab={activeTab}>
					<OverviewTab file={fileData} />
				</TabContent>

				<TabContent id="sidecars" activeTab={activeTab}>
					<SidecarsTab file={fileData} />
				</TabContent>

				<TabContent id="instances" activeTab={activeTab}>
					<InstancesTab file={fileData} />
				</TabContent>

				{isDev && (
					<TabContent id="chat" activeTab={activeTab}>
						<ChatTab />
					</TabContent>
				)}

				{isDev && (
					<TabContent id="activity" activeTab={activeTab}>
						<ActivityTab />
					</TabContent>
				)}

				<TabContent id="details" activeTab={activeTab}>
					<DetailsTab file={fileData} />
				</TabContent>
			</div>
		</>
	);
}

function OverviewTab({file}: {file: File}) {
	const formatDate = (dateStr: string) => {
		const date = new Date(dateStr);
		return date.toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	};

	// Tag mutations
	const applyTag = useLibraryMutation('tags.apply');

	// AI Processing mutations
	const extractText = useLibraryMutation('media.ocr.extract');
	const transcribeAudio = useLibraryMutation('media.speech.transcribe');
	const generateSplat = useLibraryMutation('media.splat.generate');
	const regenerateThumbnail = useLibraryMutation(
		'media.thumbnail.regenerate'
	);
	const generateThumbstrip = useLibraryMutation('media.thumbstrip.generate');
	const generateProxy = useLibraryMutation('media.proxy.generate');

	// Job tracking for long-running operations
	const {jobs} = useJobsContext();
	const isSpeechJobRunning = jobs.some(
		(job) =>
			job.name === 'speech_to_text' &&
			(job.status === 'running' || job.status === 'queued')
	);

	// Check content kind for available actions
	const isImage = getContentKind(file) === 'image';
	const isVideo = getContentKind(file) === 'video';
	const isAudio = getContentKind(file) === 'audio';
	const hasText = file?.content_identity?.text_content;

	const contentKind = getContentKind(file);
	const fileKind =
		contentKind && contentKind !== 'unknown'
			? contentKind
			: file.kind === 'File'
				? file.extension || 'File'
				: file.kind;

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
			{/* Thumbnail */}
			<div className="flex w-full justify-center px-4">
				<FileComponent.Thumb
					file={file}
					size={200}
					iconScale={0.6}
					className="w-full max-w-full"
				/>
			</div>

			{/* File name */}
			<div className="px-2 text-center">
				<h4 className="text-sidebar-ink truncate text-sm font-semibold">
					{file.name}
					{file.extension ? `.${file.extension}` : ''}
				</h4>
				<p className="text-sidebar-inkDull mt-1 text-xs">{fileKind}</p>
			</div>

			<Divider />

			{/* Details */}
			<Section title="Details" icon={Info}>
				<InfoRow label="Size" value={formatBytes(file.size)} />
				{file.kind === 'File' && file.extension && (
					<InfoRow label="Extension" value={file.extension} />
				)}
				<InfoRow label="Kind" value={fileKind} />
			</Section>

			{/* Dates */}
			<Section title="Dates" icon={Calendar}>
				{/* Show capture date for media files */}
				{file.video_media_data?.date_captured && (
					<InfoRow
						label="Captured"
						value={formatDate(file.video_media_data.date_captured)}
					/>
				)}
				{file.image_media_data?.date_taken && (
					<InfoRow
						label="Taken"
						value={formatDate(file.image_media_data.date_taken)}
					/>
				)}
				<InfoRow label="Created" value={formatDate(file.created_at)} />
				<InfoRow
					label="Modified"
					value={formatDate(file.modified_at)}
				/>
				{file.accessed_at && (
					<InfoRow
						label="Accessed"
						value={formatDate(file.accessed_at)}
					/>
				)}
			</Section>

			{/* Image Metadata */}
			{file.image_media_data && (
				<Section title="Image Info" icon={Image}>
					<InfoRow
						label="Dimensions"
						value={`${file.image_media_data.width} × ${file.image_media_data.height}`}
					/>
					{file.image_media_data.camera_make && (
						<InfoRow
							label="Camera"
							value={`${file.image_media_data.camera_make} ${file.image_media_data.camera_model || ''}`}
						/>
					)}
					{file.image_media_data.lens_model && (
						<InfoRow
							label="Lens"
							value={file.image_media_data.lens_model}
						/>
					)}
					{file.image_media_data.iso && (
						<InfoRow
							label="ISO"
							value={String(file.image_media_data.iso)}
						/>
					)}
					{file.image_media_data.focal_length && (
						<InfoRow
							label="Focal Length"
							value={file.image_media_data.focal_length}
						/>
					)}
					{file.image_media_data.aperture && (
						<InfoRow
							label="Aperture"
							value={file.image_media_data.aperture}
						/>
					)}
					{file.image_media_data.shutter_speed && (
						<InfoRow
							label="Shutter Speed"
							value={file.image_media_data.shutter_speed}
						/>
					)}
				</Section>
			)}

			{/* Video Metadata */}
			{file.video_media_data && (
				<Section title="Video Info" icon={Image}>
					<InfoRow
						label="Resolution"
						value={`${file.video_media_data.width} × ${file.video_media_data.height}`}
					/>
					{file.video_media_data.duration_seconds && (
						<InfoRow
							label="Duration"
							value={`${Math.floor(file.video_media_data.duration_seconds / 60)}:${String(Math.floor(file.video_media_data.duration_seconds % 60)).padStart(2, '0')}`}
						/>
					)}
					{file.video_media_data.codec && (
						<InfoRow
							label="Codec"
							value={file.video_media_data.codec}
						/>
					)}
					{file.video_media_data.bit_rate && (
						<InfoRow
							label="Bitrate"
							value={`${Math.round(file.video_media_data.bit_rate / 1000000)} Mbps`}
						/>
					)}
					{file.video_media_data.fps_num &&
						file.video_media_data.fps_den &&
						file.video_media_data.fps_den !== 0 && (
							<InfoRow
								label="Frame Rate"
								value={`${Math.round((file.video_media_data.fps_num / file.video_media_data.fps_den) * 100) / 100} fps`}
							/>
						)}
					{file.video_media_data.audio_codec && (
						<InfoRow
							label="Audio"
							value={`${file.video_media_data.audio_codec} · ${file.video_media_data.audio_channels || ''}`}
						/>
					)}
				</Section>
			)}

			{/* Audio Metadata */}
			{file.audio_media_data && (
				<Section title="Audio Info" icon={Image}>
					{file.audio_media_data.artist && (
						<InfoRow
							label="Artist"
							value={file.audio_media_data.artist}
						/>
					)}
					{file.audio_media_data.album && (
						<InfoRow
							label="Album"
							value={file.audio_media_data.album}
						/>
					)}
					{file.audio_media_data.title && (
						<InfoRow
							label="Title"
							value={file.audio_media_data.title}
						/>
					)}
					{file.audio_media_data.duration_seconds && (
						<InfoRow
							label="Duration"
							value={`${Math.floor(file.audio_media_data.duration_seconds / 60)}:${String(Math.floor(file.audio_media_data.duration_seconds % 60)).padStart(2, '0')}`}
						/>
					)}
					{file.audio_media_data.codec && (
						<InfoRow
							label="Codec"
							value={file.audio_media_data.codec}
						/>
					)}
					{file.audio_media_data.bit_rate && (
						<InfoRow
							label="Bitrate"
							value={`${Math.round(file.audio_media_data.bit_rate / 1000)} kbps`}
						/>
					)}
					{file.audio_media_data.genre && (
						<InfoRow
							label="Genre"
							value={file.audio_media_data.genre}
						/>
					)}
					{file.audio_media_data.year && (
						<InfoRow
							label="Year"
							value={String(file.audio_media_data.year)}
						/>
					)}
				</Section>
			)}

			{/* Storage */}
			<Section title="Storage" icon={HardDrive}>
				<InfoRow
					label="Path"
					value={
						'Physical' in file.sd_path
							? String(file.sd_path.Physical.path)
							: 'Cloud' in file.sd_path
								? String(file.sd_path.Cloud.path)
								: 'Content'
					}
				/>
				<InfoRow label="Local" value={file.is_local ? 'Yes' : 'No'} />
			</Section>

			{/* Tags */}
			<Section title="Tags" icon={TagIcon}>
				<div className="flex flex-wrap gap-1.5">
					{file.tags &&
						file.tags.length > 0 &&
						file.tags.map((tag) => (
							<Tag
								key={tag.id}
								color={tag.color || '#3B82F6'}
								size="sm"
							>
								{tag.canonical_name}
							</Tag>
						))}

					{/* Add Tag Button */}
					<TagSelectorButton
						onSelect={async (tag) => {
							// Use content-based tagging by default (tags all instances)
							// Fall back to entry-based if no content identity
							await applyTag.mutateAsync({
								targets: file.content_identity?.uuid
									? {
											type: 'Content',
											ids: [file.content_identity.uuid]
										}
									: {
											type: 'Entry',
											ids: [parseInt(file.id)]
										},
								tag_ids: [tag.id],
								source: 'User',
								confidence: 1.0
							});
						}}
						contextTags={file.tags || []}
						fileId={file.id}
						contentId={file.content_identity?.uuid}
						trigger={
							<button className="bg-app-box hover:bg-app-hover border-app-line text-ink-dull hover:text-ink rounded-full border px-2 py-0.5 text-xs font-medium transition-colors">
								+ Add tags
							</button>
						}
					/>
				</div>
			</Section>

			{/* AI Processing */}
			{(isImage || isVideo || isAudio) && (
				<Section title="AI Processing" icon={Sparkle}>
					<div className="flex flex-col gap-2">
						{/* OCR for images */}
						{isImage && (
							<button
								onClick={() => {
									console.log(
										'Extract text clicked for file:',
										file.id
									);
									extractText.mutate(
										{
											entry_uuid: file.id,
											languages: ['eng'],
											force: false
										},
										{
											onSuccess: (data) => {
												console.log(
													'OCR success:',
													data
												);
											},
											onError: (error) => {
												console.error(
													'OCR error:',
													error
												);
											}
										}
									);
								}}
								disabled={extractText.isPending}
								className={clsx(
									'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
									'bg-app-box hover:bg-app-hover border-app-line border',
									extractText.isPending &&
										'cursor-not-allowed opacity-50'
								)}
							>
								<TextAa size={4} weight="bold" />
								<span>
									{extractText.isPending
										? 'Extracting...'
										: 'Extract Text (OCR)'}
								</span>
							</button>
						)}

						{/* Gaussian Splat for images */}
						{isImage && (
							<button
								onClick={() => {
									console.log(
										'Generate splat clicked for file:',
										file.id
									);
									generateSplat.mutate(
										{
											entry_uuid: file.id,
											model_path: null
										},
										{
											onSuccess: (data) => {
												console.log(
													'Splat generation success:',
													data
												);
											},
											onError: (error) => {
												console.error(
													'Splat generation error:',
													error
												);
											}
										}
									);
								}}
								disabled={generateSplat.isPending}
								className={clsx(
									'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
									'bg-app-box hover:bg-app-hover border-app-line border',
									generateSplat.isPending &&
										'cursor-not-allowed opacity-50'
								)}
							>
								<Cube size={4} weight="bold" />
								<span>
									{generateSplat.isPending
										? 'Generating...'
										: 'Generate 3D Splat'}
								</span>
							</button>
						)}

						{/* Speech-to-text for audio/video */}
						{(isVideo || isAudio) && (
							<button
								onClick={() => {
									console.log(
										'Transcribe clicked for file:',
										file.id
									);
									transcribeAudio.mutate(
										{
											entry_uuid: file.id,
											model: 'base',
											language: null
										},
										{
											onSuccess: (data) => {
												console.log(
													'Transcription success:',
													data
												);
											},
											onError: (error) => {
												console.error(
													'Transcription error:',
													error
												);

												// Check if it's a feature-disabled error
												const errorMessage =
													error instanceof Error
														? error.message
														: String(error);
												if (
													errorMessage.includes(
														'feature is not enabled'
													) ||
													errorMessage.includes(
														'--features ffmpeg'
													)
												) {
													toast.error({
														title: 'Feature Not Available',
														body: 'Speech-to-text requires FFmpeg. Please rebuild the daemon with --features ffmpeg,heif or use `cargo daemon`'
													});
												} else {
													toast.error({
														title: 'Transcription Failed',
														body: errorMessage
													});
												}
											}
										}
									);
								}}
								disabled={
									transcribeAudio.isPending ||
									isSpeechJobRunning
								}
								className={clsx(
									'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
									'bg-app-box hover:bg-app-hover border-app-line border',
									(transcribeAudio.isPending ||
										isSpeechJobRunning) &&
										'cursor-not-allowed opacity-50'
								)}
							>
								<Microphone size={4} weight="bold" />
								<span>
									{transcribeAudio.isPending ||
									isSpeechJobRunning
										? 'Transcribing...'
										: 'Generate Subtitles'}
								</span>
							</button>
						)}

						{/* Regenerate thumbnails */}
						{(isImage || isVideo) && (
							<button
								onClick={() => {
									console.log(
										'Regenerate thumbnails clicked for file:',
										file.id
									);
									regenerateThumbnail.mutate(
										{
											entry_uuid: file.id,
											variants: [
												'grid@1x',
												'grid@2x',
												'detail@1x'
											],
											force: true
										},
										{
											onSuccess: (data) => {
												console.log(
													'Thumbnail regeneration success:',
													data
												);
											},
											onError: (error) => {
												console.error(
													'Thumbnail regeneration error:',
													error
												);

												const errorMessage =
													error instanceof Error
														? error.message
														: String(error);
												if (
													errorMessage.includes(
														'feature is not enabled'
													) ||
													errorMessage.includes(
														'--features ffmpeg'
													) ||
													errorMessage.includes(
														'FFmpeg feature'
													)
												) {
													toast.error({
														title: 'Feature Not Available',
														body: 'Video thumbnail generation requires FFmpeg. Please rebuild the daemon with --features ffmpeg or use `cargo daemon`'
													});
												} else {
													toast.error({
														title: 'Thumbnail Generation Failed',
														body: errorMessage
													});
												}
											}
										}
									);
								}}
								disabled={regenerateThumbnail.isPending}
								className={clsx(
									'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
									'bg-app-box hover:bg-app-hover border-app-line border',
									regenerateThumbnail.isPending &&
										'cursor-not-allowed opacity-50'
								)}
							>
								<ArrowsClockwise size={4} weight="bold" />
								<span>
									{regenerateThumbnail.isPending
										? 'Generating...'
										: 'Regenerate Thumbnails'}
								</span>
							</button>
						)}

						{/* Generate thumbstrip (for videos) */}
						{isVideo && (
							<button
								onClick={() => {
									console.log(
										'Generate thumbstrip clicked for file:',
										file.id
									);
									generateThumbstrip.mutate(
										{
											entry_uuid: file.id,
											variants: [
												'thumbstrip_preview',
												'thumbstrip_detailed'
											],
											force: false
										},
										{
											onSuccess: (data) => {
												console.log(
													'Thumbstrip generation success:',
													data
												);
											},
											onError: (error) => {
												console.error(
													'Thumbstrip generation error:',
													error
												);

												const errorMessage =
													error instanceof Error
														? error.message
														: String(error);
												if (
													errorMessage.includes(
														'feature is not enabled'
													) ||
													errorMessage.includes(
														'--features ffmpeg'
													)
												) {
													toast.error({
														title: 'Feature Not Available',
														body: 'Thumbstrip generation requires FFmpeg. Please rebuild the daemon with --features ffmpeg,heif or use `cargo daemon`'
													});
												} else {
													toast.error({
														title: 'Thumbstrip Generation Failed',
														body: errorMessage
													});
												}
											}
										}
									);
								}}
								disabled={generateThumbstrip.isPending}
								className={clsx(
									'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
									'bg-app-box hover:bg-app-hover border-app-line border',
									generateThumbstrip.isPending &&
										'cursor-not-allowed opacity-50'
								)}
							>
								<FilmStrip size={4} weight="bold" />
								<span>
									{generateThumbstrip.isPending
										? 'Generating...'
										: 'Generate Thumbstrip'}
								</span>
							</button>
						)}

						{/* Generate proxy (for videos) */}
						{isVideo && (
							<button
								onClick={() => {
									console.log(
										'Generate proxy clicked for file:',
										file.id
									);
									generateProxy.mutate(
										{
											entry_uuid: file.id,
											resolution: 'scrubbing', // Fast scrubbing proxy
											force: false,
											use_hardware_accel: true
										},
										{
											onSuccess: (data) => {
												console.log(
													'Proxy generation success:',
													data
												);
											},
											onError: (error) => {
												console.error(
													'Proxy generation error:',
													error
												);
											}
										}
									);
								}}
								disabled={generateProxy.isPending}
								className={clsx(
									'flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors',
									'bg-app-box hover:bg-app-hover border-app-line border',
									generateProxy.isPending &&
										'cursor-not-allowed opacity-50'
								)}
							>
								<VideoCamera size={4} weight="bold" />
								<span>
									{generateProxy.isPending
										? 'Encoding...'
										: 'Generate Scrubbing Proxy'}
								</span>
							</button>
						)}

						{/* Show extracted text if available */}
						{hasText && (
							<div className="bg-app-box/40 border-app-line/50 mt-2 rounded-lg border p-3">
								<div className="mb-2 flex items-center gap-2">
									<span className="text-accent">
										<TextAa size={16} weight="bold" />
									</span>
									<span className="text-sidebar-ink text-xs font-medium">
										Extracted Text
									</span>
								</div>
								<pre className="text-sidebar-inkDull no-scrollbar max-h-40 overflow-y-auto whitespace-pre-wrap text-xs">
									{file.content_identity.text_content}
								</pre>
							</div>
						)}
					</div>
				</Section>
			)}
		</div>
	);
}

function SidecarsTab({file}: {file: File}) {
	const sidecars = file.sidecars || [];
	const platform = usePlatform();
	const {buildSidecarUrl, libraryId} = useServer();

	// Helper to get sidecar URL
	const getSidecarUrl = (sidecar: any) => {
		if (!file.content_identity) return null;

		return buildSidecarUrl(
			file.content_identity.uuid,
			sidecar.kind,
			sidecar.variant,
			sidecar.format
		);
	};

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll px-2 pb-10 pt-2">
			<p className="text-sidebar-inkDull text-xs">
				Derivative files and associated content generated by Spacedrive
			</p>

			{sidecars.length === 0 ? (
				<div className="text-sidebar-inkDull flex items-center justify-center py-8 text-xs">
					No sidecars generated yet
				</div>
			) : (
				<div className="space-y-2">
					{sidecars.map((sidecar, i) => (
						<SidecarItem
							key={i}
							sidecar={sidecar}
							file={file}
							sidecarUrl={getSidecarUrl(sidecar)}
							platform={platform}
							libraryId={libraryId}
						/>
					))}
				</div>
			)}
		</div>
	);
}

function SidecarItem({
	sidecar,
	file,
	sidecarUrl,
	platform,
	libraryId
}: {
	sidecar: any;
	file: File;
	sidecarUrl: string | null;
	platform: ReturnType<typeof usePlatform>;
	libraryId: string | null;
}) {
	const isImage =
		(sidecar.kind === 'thumb' || sidecar.kind === 'thumbstrip') &&
		(sidecar.format === 'webp' ||
			sidecar.format === 'jpg' ||
			sidecar.format === 'png');

	// Get appropriate Spacedrive icon based on sidecar format/kind
	const getSidecarIcon = () => {
		const format = String(sidecar.format).toLowerCase();

		// PLY files (3D mesh) use Mesh icon
		if (format === 'ply') {
			return getIcon('Mesh', true);
		}

		// Text files use Text icon
		if (format === 'text' || format === 'txt' || format === 'srt') {
			return getIcon('Text', true);
		}

		// Thumbs/thumbstrips use Image icon
		if (sidecar.kind === 'thumb' || sidecar.kind === 'thumbstrip') {
			return getIcon('Image', true);
		}

		// Default to Document icon
		return getIcon('Document', true);
	};

	const sidecarIcon = getSidecarIcon();

	const contextMenu = useContextMenu({
		items: [
			{
				icon: MagnifyingGlass,
				label: 'Show in Finder',
				onClick: async () => {
					if (
						platform.getSidecarPath &&
						platform.revealFile &&
						file.content_identity &&
						libraryId
					) {
						try {
							// Convert "text" format to "txt" extension (matches actual file on disk)
							const format =
								sidecar.format === 'text'
									? 'txt'
									: sidecar.format;
							const sidecarPath = await platform.getSidecarPath(
								libraryId,
								file.content_identity.uuid,
								sidecar.kind,
								sidecar.variant,
								format
							);

							await platform.revealFile(sidecarPath);
						} catch (err) {
							console.error('Failed to reveal sidecar:', err);
						}
					}
				},
				condition: () =>
					!!platform.getSidecarPath &&
					!!platform.revealFile &&
					!!file.content_identity &&
					!!libraryId
			},
			{
				icon: Trash,
				label: 'Delete Sidecar',
				onClick: () => {
					console.log('Delete sidecar:', sidecar);
					// TODO: Implement sidecar deletion
				},
				variant: 'danger' as const
			}
		]
	});

	const handleContextMenu = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		await contextMenu.show(e);
	};

	return (
		<div
			onContextMenu={handleContextMenu}
			className="bg-app-box/40 border-app-line/50 hover:bg-app-box/60 flex cursor-default items-start gap-3 rounded-lg border p-2.5 transition-colors"
		>
			{/* Preview thumbnail for image sidecars */}
			{isImage && sidecarUrl ? (
				<div className="bg-app-box border-app-line size-12 shrink-0 overflow-hidden rounded border">
					<img
						src={sidecarUrl}
						alt={`${sidecar.variant} preview`}
						className="h-full w-full object-cover"
						onError={(e) => {
							// Fallback to icon on error
							e.currentTarget.style.display = 'none';
							if (e.currentTarget.nextElementSibling) {
								(
									e.currentTarget
										.nextElementSibling as HTMLElement
								).style.display = 'flex';
							}
						}}
					/>
					<div className="hidden h-full w-full items-center justify-center">
						<img
							src={sidecarIcon}
							alt=""
							className="size-6 object-contain"
						/>
					</div>
				</div>
			) : (
				<div className="bg-app-box border-app-line flex size-12 shrink-0 items-center justify-center rounded border">
					<img
						src={sidecarIcon}
						alt=""
						className="size-6 object-contain"
					/>
				</div>
			)}

			<div className="min-w-0 flex-1">
				<div className="text-sidebar-ink text-xs font-medium">
					{String(sidecar.kind)}
				</div>
				<div className="text-sidebar-inkDull text-[11px]">
					{String(sidecar.variant)} · {formatBytes(sidecar.size)}
				</div>
				<div className="text-sidebar-inkDull/70 mt-0.5 text-[10px]">
					{String(sidecar.format).toUpperCase()}
				</div>
			</div>
			{/* <span
				className={clsx(
					"text-[10px] font-semibold px-2 py-0.5 rounded-full shrink-0",
					sidecar.status === "ready" && "bg-accent/20 text-accent",
					sidecar.status === "pending" &&
						"bg-sidebar-inkDull/20 text-sidebar-inkDull",
				)}
			>
				{String(sidecar.status)}
			</span> */}
		</div>
	);
}

function InstancesTab({file}: {file: File}) {
	// Query for alternate instances with full File data
	const instancesQuery = useNormalizedQuery<
		{entry_uuid: string},
		{instances: File[]; total_count: number}
	>({
		wireMethod: 'query:files.alternate_instances',
		input: {entry_uuid: file?.id || ''},
		enabled: !!file?.id && !!file?.content_identity
	});

	const instances = instancesQuery.data?.instances || [];

	// Query devices to get proper names and icons
	const devicesQuery = useNormalizedQuery<any, any[]>({
		wireMethod: 'query:devices.list',
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true
		},
		resourceType: 'device'
	});

	const devices = devicesQuery.data || [];

	// Group instances by device_slug
	const instancesByDevice = instances.reduce(
		(acc, instance) => {
			let deviceSlug = 'unknown';
			if ('Physical' in instance.sd_path) {
				deviceSlug = instance.sd_path.Physical.device_slug;
			} else if ('Cloud' in instance.sd_path) {
				deviceSlug = 'cloud';
			}

			if (!acc[deviceSlug]) {
				acc[deviceSlug] = [];
			}
			acc[deviceSlug].push(instance);
			return acc;
		},
		{} as Record<string, File[]>
	);

	const getDeviceName = (deviceSlug: string) => {
		const device = devices.find((d) => d.slug === deviceSlug);
		return device?.name || deviceSlug;
	};

	const getDeviceInfo = (deviceSlug: string) => {
		return devices.find((d) => d.slug === deviceSlug);
	};

	if (instancesQuery.isLoading) {
		return (
			<div className="text-sidebar-inkDull flex items-center justify-center py-8 text-xs">
				Loading instances...
			</div>
		);
	}

	if (!file.content_identity) {
		return (
			<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll px-2 pb-10 pt-2">
				<p className="text-sidebar-inkDull text-xs">
					This file has not been content-hashed yet. Instances will
					appear after indexing completes.
				</p>
			</div>
		);
	}

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll px-2 pb-10 pt-2">
			<p className="text-sidebar-inkDull text-xs">
				All copies of this file across your devices and locations
			</p>

			{instances.length === 0 || instances.length === 1 ? (
				<div className="text-sidebar-inkDull flex items-center justify-center py-8 text-xs">
					No alternate instances found
				</div>
			) : (
				<div className="space-y-4">
					{Object.entries(instancesByDevice).map(
						([deviceSlug, deviceInstances]) => {
							const deviceInfo = getDeviceInfo(deviceSlug);
							const deviceName = getDeviceName(deviceSlug);

							return (
								<div key={deviceSlug} className="space-y-1">
									{/* Device Header */}
									<div className="flex items-center gap-2 px-2">
										<img
											src={getDeviceIcon(deviceInfo)}
											className="size-4 shrink-0"
											alt=""
										/>
										<span className="text-sidebar-ink text-xs font-semibold">
											{deviceName}
										</span>
										<div className="flex-1" />
										<div className="bg-app-box border-app-line text-sidebar-inkDull flex size-5 items-center justify-center rounded-full border text-[10px] font-semibold">
											{deviceInstances.length}
										</div>
									</div>

									{/* List of instances */}
									<div className="space-y-0.5">
										{deviceInstances.map((instance, i) => (
											<InstanceRow
												key={i}
												instance={instance}
											/>
										))}
									</div>
								</div>
							);
						}
					)}
				</div>
			)}
		</div>
	);
}

function InstanceRow({instance}: {instance: File}) {
	const getPathDisplay = (sdPath: typeof instance.sd_path) => {
		if ('Physical' in sdPath) {
			return sdPath.Physical.path;
		} else if ('Cloud' in sdPath) {
			return sdPath.Cloud.path;
		} else {
			return 'Content';
		}
	};

	const formatDate = (dateStr: string) => {
		const date = new Date(dateStr);
		return date.toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric'
		});
	};

	return (
		<div
			className="hover:bg-app-box/40 flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 transition-colors"
			title={getPathDisplay(instance.sd_path)}
		>
			{/* Thumbnail */}
			<div className="flex-shrink-0 [&_*]:!rounded-[3px]">
				<FileComponent.Thumb file={instance} size={20} />
			</div>

			{/* File info */}
			<div className="flex min-w-0 flex-1 items-center gap-2">
				<span className="text-sidebar-ink truncate text-xs">
					{instance.name}
					{instance.extension && `.${instance.extension}`}
				</span>
			</div>

			{/* Metadata */}
			<div className="flex shrink-0 items-center gap-2">
				{/* Tags */}
				{instance.tags && instance.tags.length > 0 && (
					<div
						className="flex items-center gap-0.5"
						title={instance.tags
							.map((t) => t.canonical_name)
							.join(', ')}
					>
						{instance.tags.slice(0, 3).map((tag) => (
							<div
								key={tag.id}
								className="size-1.5 rounded-full"
								style={{
									backgroundColor: tag.color || '#3B82F6'
								}}
							/>
						))}
						{instance.tags.length > 3 && (
							<span className="text-ink-faint text-[9px] font-medium">
								+{instance.tags.length - 3}
							</span>
						)}
					</div>
				)}

				{/* Modified date */}
				<span className="text-sidebar-inkDull text-[10px]">
					{formatDate(instance.modified_at)}
				</span>

				{/* Size */}
				<span className="text-sidebar-inkDull min-w-[50px] text-right text-[10px]">
					{formatBytes(instance.size)}
				</span>

				{/* Local indicator */}
				<div
					className={clsx(
						'size-1.5 rounded-full',
						instance.is_local
							? 'bg-accent'
							: 'bg-sidebar-inkDull/40'
					)}
					title={instance.is_local ? 'Available locally' : 'Remote'}
				/>
			</div>
		</div>
	);
}

function ChatTab() {
	const [message, setMessage] = useState('');

	const messages = [
		{
			id: 1,
			sender: 'Sarah',
			avatar: 'S',
			content: 'Can you check if this photo is also on the NAS?',
			time: '2:34 PM',
			isUser: false
		},
		{
			id: 2,
			sender: 'You',
			avatar: 'J',
			content: "Yeah, it's synced. Shows 3 instances across devices.",
			time: '2:35 PM',
			isUser: true
		},
		{
			id: 3,
			sender: 'AI Assistant',
			avatar: '',
			content:
				'I found 2 similar photos in your library from the same location. Would you like me to create a collection?',
			time: '2:36 PM',
			isUser: false,
			isAI: true,
			unread: true
		},
		{
			id: 4,
			sender: 'Sarah',
			avatar: 'S',
			content: 'Perfect, thanks! Can you share the collection with me?',
			time: '2:37 PM',
			isUser: false,
			unread: true
		},
		{
			id: 5,
			sender: 'Alex',
			avatar: 'A',
			content: 'I just tagged this as Summer 2025 btw',
			time: '2:38 PM',
			isUser: false,
			unread: true
		}
	];

	return (
		<div className="flex h-full flex-col">
			{/* Messages */}
			<div className="flex-1 space-y-3 overflow-y-auto px-2 pt-2">
				{messages.map((msg) => (
					<div
						key={msg.id}
						className={clsx(
							'flex gap-2',
							msg.isUser ? 'flex-row-reverse' : 'flex-row'
						)}
					>
						{/* Avatar */}
						<div
							className={clsx(
								'flex size-6 shrink-0 items-center justify-center rounded-full text-[10px] font-bold',
								msg.isAI
									? 'bg-accent/20 text-accent'
									: msg.isUser
										? 'bg-sidebar-selected text-sidebar-ink'
										: 'bg-app-box text-sidebar-inkDull'
							)}
						>
							{msg.avatar}
						</div>

						{/* Message bubble */}
						<div
							className={clsx(
								'flex max-w-[75%] flex-col',
								msg.isUser ? 'items-end' : 'items-start'
							)}
						>
							<div
								className={clsx(
									'rounded-lg px-2.5 py-1.5',
									msg.isAI
										? 'bg-accent/10 border-accent/20 border'
										: msg.isUser
											? 'bg-sidebar-selected/60'
											: 'bg-app-box/60',
									msg.unread && 'ring-accent/50 ring-1'
								)}
							>
								{!msg.isUser && (
									<div
										className={clsx(
											'mb-0.5 text-[10px] font-semibold',
											msg.isAI
												? 'text-accent'
												: 'text-sidebar-inkDull'
										)}
									>
										{msg.sender}
									</div>
								)}
								<p className="text-sidebar-ink text-xs leading-relaxed">
									{msg.content}
								</p>
							</div>
							<span className="text-sidebar-inkDull mt-0.5 px-1 text-[10px]">
								{msg.time}
							</span>
						</div>
					</div>
				))}
			</div>

			{/* Input */}
			<div className="border-sidebar-line space-y-2 border-t p-2">
				<div className="flex items-end gap-1.5">
					<button
						className="hover:bg-sidebar-selected text-sidebar-inkDull hover:text-sidebar-ink rounded-lg p-1.5 transition-colors"
						title="Attach file"
					>
						<Paperclip size={4} weight="bold" />
					</button>

					<div className="bg-app-box border-app-line flex flex-1 items-center gap-1.5 rounded-lg border px-2 py-1.5">
						<input
							type="text"
							value={message}
							onChange={(e) => setMessage(e.target.value)}
							placeholder="Type a message..."
							className="text-sidebar-ink placeholder:text-sidebar-inkDull flex-1 bg-transparent text-xs outline-none"
						/>
					</div>

					<button
						className="bg-accent hover:bg-accent/90 rounded-lg p-1.5 text-white transition-colors"
						title="Send message"
					>
						<PaperPlaneRight size={4} weight="bold" />
					</button>
				</div>

				<div className="flex gap-1">
					<button className="text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 flex items-center gap-1 rounded-md px-2 py-1 text-[10px] font-medium transition-colors">
						<Sparkle size={3} weight="bold" />
						Ask AI
					</button>
					<button className="text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md px-2 py-1 text-[10px] font-medium transition-colors">
						Share File
					</button>
					<button className="text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md px-2 py-1 text-[10px] font-medium transition-colors">
						Create Task
					</button>
				</div>
			</div>
		</div>
	);
}

function ActivityTab() {
	const activity = [
		{action: 'Synced to NAS', time: '2 min ago', device: 'MacBook Pro'},
		{action: 'Uploaded to S3', time: '1 hour ago', device: 'MacBook Pro'},
		{
			action: 'Thumbnail generated',
			time: '2 hours ago',
			device: 'MacBook Pro'
		},
		{action: "Tagged as 'Travel'", time: '3 hours ago', device: 'iPhone'},
		{action: 'Created', time: 'Jan 15, 2025', device: 'iPhone'}
	];

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll px-2 pb-10 pt-2">
			<p className="text-sidebar-inkDull text-xs">
				History of changes and sync operations
			</p>

			<div className="space-y-0.5">
				{activity.map((item, i) => (
					<div
						key={i}
						className="hover:bg-app-box/40 flex items-start gap-3 rounded-lg p-2 transition-colors"
					>
						<span className="text-sidebar-inkDull mt-0.5 shrink-0">
							<ClockCounterClockwise size={16} weight="bold" />
						</span>
						<div className="min-w-0 flex-1">
							<div className="text-sidebar-ink text-xs">
								{item.action}
							</div>
							<div className="text-sidebar-inkDull mt-0.5 text-[11px]">
								{item.time} · {item.device}
							</div>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

function DetailsTab({file}: {file: File}) {
	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
			{/* Content Identity */}
			{file.content_identity && (
				<Section title="Content Identity" icon={Fingerprint}>
					<InfoRow
						label="Content Hash"
						value={
							String(file.content_identity.content_hash).slice(
								0,
								16
							) + '...'
						}
						mono
					/>
					{file.content_identity.integrity_hash && (
						<InfoRow
							label="Integrity Hash"
							value={
								String(
									file.content_identity.integrity_hash
								).slice(0, 16) + '...'
							}
							mono
						/>
					)}
					{file.content_identity.mime_type_id !== null && (
						<InfoRow
							label="MIME Type ID"
							value={String(file.content_identity.mime_type_id)}
						/>
					)}
				</Section>
			)}

			{/* Metadata */}
			<Section title="Metadata" icon={Hash}>
				<InfoRow
					label="File ID"
					value={String(file.id).slice(0, 8) + '...'}
					mono
				/>
				<InfoRow
					label="Content Kind"
					value={String(file.content_kind || 'Unknown')}
				/>
				{file.extension && (
					<InfoRow label="Extension" value={String(file.extension)} />
				)}
			</Section>

			{/* System */}
			<Section title="System" icon={DotsThree}>
				<InfoRow label="Entry Kind" value={file.kind} />
				<InfoRow label="Local" value={file.is_local ? 'Yes' : 'No'} />
				<InfoRow
					label="Instances"
					value={String((file.alternate_paths?.length || 0) + 1)}
				/>
			</Section>
		</div>
	);
}
