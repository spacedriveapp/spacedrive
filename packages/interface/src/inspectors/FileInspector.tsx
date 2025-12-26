import {
	Info,
	Tag as TagIcon,
	Calendar,
	HardDrive,
	Hash,
	Fingerprint,
	Palette,
	Image,
	ClockCounterClockwise,
	DotsThree,
	MapPin,
	ChatCircle,
	PaperPlaneRight,
	Paperclip,
	Sparkle,
	TextAa,
	Microphone,
	ArrowsClockwise,
	MagnifyingGlass,
	Trash,
	FilmStrip,
	VideoCamera,
	Cube,
} from "@phosphor-icons/react";
import { useState } from "react";
import { getContentKind } from "../components/Explorer/utils";
import {
	InfoRow,
	Tag,
	Section,
	Divider,
	Tabs,
	TabContent,
} from "../components/Inspector";
import { TagSelectorButton } from "../components/Tags";
import clsx from "clsx";
import type { File } from "@sd/ts-client";
import { useNormalizedQuery, useLibraryMutation } from "../context";
import { formatBytes } from "../components/Explorer/utils";
import { File as FileComponent } from "../components/Explorer/File";
import { useContextMenu } from "../hooks/useContextMenu";
import { usePlatform } from "../platform";
import { useServer } from "../ServerContext";
import { useJobs } from "../components/JobManager/hooks/useJobs";
import { getIcon } from "@sd/assets/util";

interface FileInspectorProps {
	file: File;
}

export function FileInspector({ file }: FileInspectorProps) {
	const [activeTab, setActiveTab] = useState("overview");

	const fileQuery = useNormalizedQuery<{ file_id: string }, File>({
		wireMethod: "query:files.by_id",
		input: { file_id: file?.id || "" },
		resourceType: "file",
		resourceId: file?.id, // Filter batch events to only this file
		enabled: !!file?.id,
	});

	const fileData = fileQuery.data || file;

	const tabs = [
		{ id: "overview", label: "Overview", icon: Info },
		{ id: "sidecars", label: "Sidecars", icon: Image },
		{ id: "instances", label: "Instances", icon: MapPin },
		{ id: "chat", label: "Chat", icon: ChatCircle, badge: 3 },
		{ id: "activity", label: "Activity", icon: ClockCounterClockwise },
		{ id: "details", label: "More", icon: DotsThree },
	];

	return (
		<>
			{/* Tabs */}
			<Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

			{/* Tab Content */}
			<div className="flex-1 overflow-hidden flex flex-col mt-2.5">
				<TabContent id="overview" activeTab={activeTab}>
					<OverviewTab file={fileData} />
				</TabContent>

				<TabContent id="sidecars" activeTab={activeTab}>
					<SidecarsTab file={fileData} />
				</TabContent>

				<TabContent id="instances" activeTab={activeTab}>
					<InstancesTab file={fileData} />
				</TabContent>

				<TabContent id="chat" activeTab={activeTab}>
					<ChatTab />
				</TabContent>

				<TabContent id="activity" activeTab={activeTab}>
					<ActivityTab />
				</TabContent>

				<TabContent id="details" activeTab={activeTab}>
					<DetailsTab file={fileData} />
				</TabContent>
			</div>
		</>
	);
}

function OverviewTab({ file }: { file: File }) {
	const formatDate = (dateStr: string) => {
		const date = new Date(dateStr);
		return date.toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric",
		});
	};

	// Tag mutations
	const applyTag = useLibraryMutation("tags.apply");

	// AI Processing mutations
	const extractText = useLibraryMutation("media.ocr.extract");
	const transcribeAudio = useLibraryMutation("media.speech.transcribe");
	const generateSplat = useLibraryMutation("media.splat.generate");
	const regenerateThumbnail = useLibraryMutation(
		"media.thumbnail.regenerate",
	);
	const generateThumbstrip = useLibraryMutation("media.thumbstrip.generate");
	const generateProxy = useLibraryMutation("media.proxy.generate");

	// Job tracking for long-running operations
	const { jobs } = useJobs();
	const isSpeechJobRunning = jobs.some(
		(job) =>
			job.name === "speech_to_text" &&
			(job.status === "running" || job.status === "queued"),
	);

	// Check content kind for available actions
	const isImage = getContentKind(file) === "image";
	const isVideo = getContentKind(file) === "video";
	const isAudio = getContentKind(file) === "audio";
	const hasText = file?.content_identity?.text_content;

	const contentKind = getContentKind(file);
	const fileKind =
		contentKind && contentKind !== "unknown"
			? contentKind
			: file.kind === "File"
				? file.extension || "File"
				: file.kind;

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
			{/* Thumbnail */}
			<div className="flex justify-center w-full px-4">
				<FileComponent.Thumb
					file={file}
					size={200}
					iconScale={0.6}
					className="w-full max-w-full"
				/>
			</div>

			{/* File name */}
			<div className="px-2 text-center">
				<h4 className="text-sm font-semibold text-sidebar-ink truncate">
					{file.name}
					{file.extension ? `.${file.extension}` : ""}
				</h4>
				<p className="text-xs text-sidebar-inkDull mt-1">{fileKind}</p>
			</div>

			<Divider />

			{/* Details */}
			<Section title="Details" icon={Info}>
				<InfoRow label="Size" value={formatBytes(file.size)} />
				{file.kind === "File" && file.extension && (
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
							value={`${file.image_media_data.camera_make} ${file.image_media_data.camera_model || ""}`}
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
							value={`${Math.floor(file.video_media_data.duration_seconds / 60)}:${String(Math.floor(file.video_media_data.duration_seconds % 60)).padStart(2, "0")}`}
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
							value={`${file.video_media_data.audio_codec} · ${file.video_media_data.audio_channels || ""}`}
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
							value={`${Math.floor(file.audio_media_data.duration_seconds / 60)}:${String(Math.floor(file.audio_media_data.duration_seconds % 60)).padStart(2, "0")}`}
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
						"Physical" in file.sd_path
							? String(file.sd_path.Physical.path)
							: "Cloud" in file.sd_path
								? String(file.sd_path.Cloud.path)
								: "Content"
					}
				/>
				<InfoRow label="Local" value={file.is_local ? "Yes" : "No"} />
			</Section>

			{/* Tags */}
			<Section title="Tags" icon={TagIcon}>
				<div className="flex flex-wrap gap-1.5">
					{file.tags &&
						file.tags.length > 0 &&
						file.tags.map((tag) => (
							<Tag
								key={tag.id}
								color={tag.color || "#3B82F6"}
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
											type: "Content",
											ids: [file.content_identity.uuid],
										}
									: {
											type: "Entry",
											ids: [parseInt(file.id)],
										},
								tag_ids: [tag.id],
								source: "User",
								confidence: 1.0,
							});
						}}
						contextTags={file.tags || []}
						fileId={file.id}
						contentId={file.content_identity?.uuid}
						trigger={
							<button className="px-2 py-0.5 text-xs font-medium rounded-full bg-app-box hover:bg-app-hover border border-app-line text-ink-dull hover:text-ink transition-colors">
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
										"Extract text clicked for file:",
										file.id,
									);
									extractText.mutate(
										{
											entry_uuid: file.id,
											languages: ["eng"],
											force: false,
										},
										{
											onSuccess: (data) => {
												console.log(
													"OCR success:",
													data,
												);
											},
											onError: (error) => {
												console.error(
													"OCR error:",
													error,
												);
											},
										},
									);
								}}
								disabled={extractText.isPending}
								className={clsx(
									"flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors",
									"bg-app-box hover:bg-app-hover border border-app-line",
									extractText.isPending &&
										"opacity-50 cursor-not-allowed",
								)}
							>
								<TextAa size={4} weight="bold" />
								<span>
									{extractText.isPending
										? "Extracting..."
										: "Extract Text (OCR)"}
								</span>
							</button>
						)}

						{/* Gaussian Splat for images */}
						{isImage && (
							<button
								onClick={() => {
									console.log(
										"Generate splat clicked for file:",
										file.id,
									);
									generateSplat.mutate(
										{
											entry_uuid: file.id,
											model_path: null,
										},
										{
											onSuccess: (data) => {
												console.log(
													"Splat generation success:",
													data,
												);
											},
											onError: (error) => {
												console.error(
													"Splat generation error:",
													error,
												);
											},
										},
									);
								}}
								disabled={generateSplat.isPending}
								className={clsx(
									"flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors",
									"bg-app-box hover:bg-app-hover border border-app-line",
									generateSplat.isPending &&
										"opacity-50 cursor-not-allowed",
								)}
							>
								<Cube size={4} weight="bold" />
								<span>
									{generateSplat.isPending
										? "Generating..."
										: "Generate 3D Splat"}
								</span>
							</button>
						)}

						{/* Speech-to-text for audio/video */}
						{(isVideo || isAudio) && (
							<button
								onClick={() => {
									console.log(
										"Transcribe clicked for file:",
										file.id,
									);
									transcribeAudio.mutate(
										{
											entry_uuid: file.id,
											model: "base",
											language: null,
										},
										{
											onSuccess: (data) => {
												console.log(
													"Transcription success:",
													data,
												);
											},
											onError: (error) => {
												console.error(
													"Transcription error:",
													error,
												);
											},
										},
									);
								}}
								disabled={
									transcribeAudio.isPending ||
									isSpeechJobRunning
								}
								className={clsx(
									"flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors",
									"bg-app-box hover:bg-app-hover border border-app-line",
									(transcribeAudio.isPending ||
										isSpeechJobRunning) &&
										"opacity-50 cursor-not-allowed",
								)}
							>
								<Microphone size={4} weight="bold" />
								<span>
									{transcribeAudio.isPending ||
									isSpeechJobRunning
										? "Transcribing..."
										: "Generate Subtitles"}
								</span>
							</button>
						)}

						{/* Regenerate thumbnails */}
						{(isImage || isVideo) && (
							<button
								onClick={() => {
									console.log(
										"Regenerate thumbnails clicked for file:",
										file.id,
									);
									regenerateThumbnail.mutate(
										{
											entry_uuid: file.id,
											variants: [
												"grid@1x",
												"grid@2x",
												"detail@1x",
											],
											force: true,
										},
										{
											onSuccess: (data) => {
												console.log(
													"Thumbnail regeneration success:",
													data,
												);
											},
											onError: (error) => {
												console.error(
													"Thumbnail regeneration error:",
													error,
												);
											},
										},
									);
								}}
								disabled={regenerateThumbnail.isPending}
								className={clsx(
									"flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors",
									"bg-app-box hover:bg-app-hover border border-app-line",
									regenerateThumbnail.isPending &&
										"opacity-50 cursor-not-allowed",
								)}
							>
								<ArrowsClockwise size={4} weight="bold" />
								<span>
									{regenerateThumbnail.isPending
										? "Generating..."
										: "Regenerate Thumbnails"}
								</span>
							</button>
						)}

						{/* Generate thumbstrip (for videos) */}
						{isVideo && (
							<button
								onClick={() => {
									console.log(
										"Generate thumbstrip clicked for file:",
										file.id,
									);
									generateThumbstrip.mutate(
										{
											entry_uuid: file.id,
											variants: [
												"thumbstrip_preview",
												"thumbstrip_detailed",
											],
											force: false,
										},
										{
											onSuccess: (data) => {
												console.log(
													"Thumbstrip generation success:",
													data,
												);
											},
											onError: (error) => {
												console.error(
													"Thumbstrip generation error:",
													error,
												);
											},
										},
									);
								}}
								disabled={generateThumbstrip.isPending}
								className={clsx(
									"flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors",
									"bg-app-box hover:bg-app-hover border border-app-line",
									generateThumbstrip.isPending &&
										"opacity-50 cursor-not-allowed",
								)}
							>
								<FilmStrip size={4} weight="bold" />
								<span>
									{generateThumbstrip.isPending
										? "Generating..."
										: "Generate Thumbstrip"}
								</span>
							</button>
						)}

						{/* Generate proxy (for videos) */}
						{isVideo && (
							<button
								onClick={() => {
									console.log(
										"Generate proxy clicked for file:",
										file.id,
									);
									generateProxy.mutate(
										{
											entry_uuid: file.id,
											resolution: "scrubbing", // Fast scrubbing proxy
											force: false,
											use_hardware_accel: true,
										},
										{
											onSuccess: (data) => {
												console.log(
													"Proxy generation success:",
													data,
												);
											},
											onError: (error) => {
												console.error(
													"Proxy generation error:",
													error,
												);
											},
										},
									);
								}}
								disabled={generateProxy.isPending}
								className={clsx(
									"flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors",
									"bg-app-box hover:bg-app-hover border border-app-line",
									generateProxy.isPending &&
										"opacity-50 cursor-not-allowed",
								)}
							>
								<VideoCamera size={4} weight="bold" />
								<span>
									{generateProxy.isPending
										? "Encoding..."
										: "Generate Scrubbing Proxy"}
								</span>
							</button>
						)}

						{/* Show extracted text if available */}
						{hasText && (
							<div className="mt-2 p-3 bg-app-box/40 rounded-lg border border-app-line/50">
								<div className="flex items-center gap-2 mb-2">
									<span className="text-accent">
										<TextAa size={16} weight="bold" />
									</span>
									<span className="text-xs font-medium text-sidebar-ink">
										Extracted Text
									</span>
								</div>
								<pre className="text-xs text-sidebar-inkDull whitespace-pre-wrap max-h-40 overflow-y-auto no-scrollbar">
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

function SidecarsTab({ file }: { file: File }) {
	const sidecars = file.sidecars || [];
	const platform = usePlatform();
	const { buildSidecarUrl, libraryId } = useServer();

	// Helper to get sidecar URL
	const getSidecarUrl = (sidecar: any) => {
		if (!file.content_identity) return null;

		return buildSidecarUrl(
			file.content_identity.uuid,
			sidecar.kind,
			sidecar.variant,
			sidecar.format,
		);
	};

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				Derivative files and associated content generated by Spacedrive
			</p>

			{sidecars.length === 0 ? (
				<div className="flex items-center justify-center py-8 text-xs text-sidebar-inkDull">
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
	libraryId,
}: {
	sidecar: any;
	file: File;
	sidecarUrl: string | null;
	platform: ReturnType<typeof usePlatform>;
	libraryId: string | null;
}) {
	const isImage =
		(sidecar.kind === "thumb" || sidecar.kind === "thumbstrip") &&
		(sidecar.format === "webp" ||
			sidecar.format === "jpg" ||
			sidecar.format === "png");

	// Get appropriate Spacedrive icon based on sidecar format/kind
	const getSidecarIcon = () => {
		const format = String(sidecar.format).toLowerCase();
		
		// PLY files (3D mesh) use Mesh icon
		if (format === "ply") {
			return getIcon("Mesh", true);
		}
		
		// Text files use Text icon
		if (format === "text" || format === "txt" || format === "srt") {
			return getIcon("Text", true);
		}
		
		// Thumbs/thumbstrips use Image icon
		if (sidecar.kind === "thumb" || sidecar.kind === "thumbstrip") {
			return getIcon("Image", true);
		}
		
		// Default to Document icon
		return getIcon("Document", true);
	};

	const sidecarIcon = getSidecarIcon();

	const contextMenu = useContextMenu({
		items: [
			{
				icon: MagnifyingGlass,
				label: "Show in Finder",
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
								sidecar.format === "text"
									? "txt"
									: sidecar.format;
							const sidecarPath = await platform.getSidecarPath(
								libraryId,
								file.content_identity.uuid,
								sidecar.kind,
								sidecar.variant,
								format,
							);

							await platform.revealFile(sidecarPath);
						} catch (err) {
							console.error("Failed to reveal sidecar:", err);
						}
					}
				},
				condition: () =>
					!!platform.getSidecarPath &&
					!!platform.revealFile &&
					!!file.content_identity &&
					!!libraryId,
			},
			{
				icon: Trash,
				label: "Delete Sidecar",
				onClick: () => {
					console.log("Delete sidecar:", sidecar);
					// TODO: Implement sidecar deletion
				},
				variant: "danger" as const,
			},
		],
	});

	const handleContextMenu = async (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		await contextMenu.show(e);
	};

	return (
		<div
			onContextMenu={handleContextMenu}
			className="flex items-start gap-3 p-2.5 bg-app-box/40 rounded-lg border border-app-line/50 hover:bg-app-box/60 transition-colors cursor-default"
		>
			{/* Preview thumbnail for image sidecars */}
			{isImage && sidecarUrl ? (
				<div className="size-12 shrink-0 rounded overflow-hidden bg-app-box border border-app-line">
					<img
						src={sidecarUrl}
						alt={`${sidecar.variant} preview`}
						className="w-full h-full object-cover"
						onError={(e) => {
							// Fallback to icon on error
							e.currentTarget.style.display = "none";
							if (e.currentTarget.nextElementSibling) {
								(
									e.currentTarget
										.nextElementSibling as HTMLElement
								).style.display = "flex";
							}
						}}
					/>
					<div className="hidden items-center justify-center w-full h-full">
						<img
							src={sidecarIcon}
							alt=""
							className="size-6 object-contain"
						/>
					</div>
				</div>
			) : (
				<div className="size-12 shrink-0 rounded bg-app-box border border-app-line flex items-center justify-center">
					<img
						src={sidecarIcon}
						alt=""
						className="size-6 object-contain"
					/>
				</div>
			)}

			<div className="flex-1 min-w-0">
				<div className="text-xs font-medium text-sidebar-ink">
					{String(sidecar.kind)}
				</div>
				<div className="text-[11px] text-sidebar-inkDull">
					{String(sidecar.variant)} · {formatBytes(sidecar.size)}
				</div>
				<div className="text-[10px] text-sidebar-inkDull/70 mt-0.5">
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

function InstancesTab({ file }: { file: File }) {
	// Query for alternate instances with full File data
	const instancesQuery = useNormalizedQuery<
		{ entry_uuid: string },
		{ instances: File[]; total_count: number }
	>({
		wireMethod: "query:files.alternate_instances",
		input: { entry_uuid: file?.id || "" },
		enabled: !!file?.id && !!file?.content_identity,
	});

	const instances = instancesQuery.data?.instances || [];

	const getPathDisplay = (sdPath: typeof file.sd_path) => {
		if ("Physical" in sdPath) {
			return sdPath.Physical.path;
		} else if ("Cloud" in sdPath) {
			return sdPath.Cloud.path;
		} else {
			return "Content";
		}
	};

	const getDeviceDisplay = (sdPath: typeof file.sd_path) => {
		if ("Physical" in sdPath) {
			return sdPath.Physical.device_slug || "Local Device";
		} else if ("Cloud" in sdPath) {
			return "Cloud Storage";
		} else {
			return "Content Addressed";
		}
	};

	const formatDate = (dateStr: string) => {
		const date = new Date(dateStr);
		return date.toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric",
		});
	};

	if (instancesQuery.isLoading) {
		return (
			<div className="flex items-center justify-center py-8 text-xs text-sidebar-inkDull">
				Loading instances...
			</div>
		);
	}

	if (!file.content_identity) {
		return (
			<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
				<p className="text-xs text-sidebar-inkDull">
					This file has not been content-hashed yet. Instances will
					appear after indexing completes.
				</p>
			</div>
		);
	}

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				All copies of this file across your devices and locations
			</p>

			{instances.length === 0 || instances.length === 1 ? (
				<div className="flex items-center justify-center py-8 text-xs text-sidebar-inkDull">
					No alternate instances found
				</div>
			) : (
				<div className="space-y-2.5">
					{instances.map((instance, i) => (
						<div
							key={i}
							className="p-2.5 bg-app-box/40 rounded-lg border border-app-line/50 hover:bg-app-box/60 transition-colors"
						>
							<div className="flex items-start gap-3">
								{/* Thumbnail */}
								<div className="shrink-0">
									<FileComponent.Thumb
										file={instance}
										size={64}
										iconScale={0.5}
										className="rounded overflow-hidden"
									/>
								</div>

								{/* Info */}
								<div className="flex-1 min-w-0 space-y-1.5">
									<div className="flex items-start justify-between gap-2">
										<div className="flex-1 min-w-0">
											<div className="text-xs font-medium text-sidebar-ink truncate">
												{instance.name}
												{instance.extension &&
													`.${instance.extension}`}
											</div>
											<div className="text-[11px] text-sidebar-inkDull mt-0.5">
												{formatBytes(instance.size)}
											</div>
										</div>
										<div
											className={clsx(
												"size-2 rounded-full shrink-0 mt-1",
												instance.is_local
													? "bg-accent"
													: "bg-sidebar-inkDull/40",
											)}
											title={
												instance.is_local
													? "Available locally"
													: "Remote"
											}
										/>
									</div>

									<div className="flex items-center gap-1.5 text-[11px] text-sidebar-inkDull">
										<MapPin size={12} weight="bold" />
										<span className="truncate">
											{getDeviceDisplay(
												instance.sd_path,
											)}
										</span>
									</div>

									<div className="text-[10px] text-sidebar-inkDull/70 font-mono truncate">
										{getPathDisplay(instance.sd_path)}
									</div>

									<div className="text-[10px] text-sidebar-inkDull/70">
										Modified{" "}
										{formatDate(instance.modified_at)}
									</div>
								</div>
							</div>
						</div>
					))}
				</div>
			)}
		</div>
	);
}

function ChatTab() {
	const [message, setMessage] = useState("");

	const messages = [
		{
			id: 1,
			sender: "Sarah",
			avatar: "S",
			content: "Can you check if this photo is also on the NAS?",
			time: "2:34 PM",
			isUser: false,
		},
		{
			id: 2,
			sender: "You",
			avatar: "J",
			content: "Yeah, it's synced. Shows 3 instances across devices.",
			time: "2:35 PM",
			isUser: true,
		},
		{
			id: 3,
			sender: "AI Assistant",
			avatar: "",
			content:
				"I found 2 similar photos in your library from the same location. Would you like me to create a collection?",
			time: "2:36 PM",
			isUser: false,
			isAI: true,
			unread: true,
		},
		{
			id: 4,
			sender: "Sarah",
			avatar: "S",
			content: "Perfect, thanks! Can you share the collection with me?",
			time: "2:37 PM",
			isUser: false,
			unread: true,
		},
		{
			id: 5,
			sender: "Alex",
			avatar: "A",
			content: "I just tagged this as Summer 2025 btw",
			time: "2:38 PM",
			isUser: false,
			unread: true,
		},
	];

	return (
		<div className="flex flex-col h-full">
			{/* Messages */}
			<div className="flex-1 overflow-y-auto px-2 pt-2 space-y-3">
				{messages.map((msg) => (
					<div
						key={msg.id}
						className={clsx(
							"flex gap-2",
							msg.isUser ? "flex-row-reverse" : "flex-row",
						)}
					>
						{/* Avatar */}
						<div
							className={clsx(
								"size-6 rounded-full shrink-0 flex items-center justify-center text-[10px] font-bold",
								msg.isAI
									? "bg-accent/20 text-accent"
									: msg.isUser
										? "bg-sidebar-selected text-sidebar-ink"
										: "bg-app-box text-sidebar-inkDull",
							)}
						>
							{msg.avatar}
						</div>

						{/* Message bubble */}
						<div
							className={clsx(
								"flex flex-col max-w-[75%]",
								msg.isUser ? "items-end" : "items-start",
							)}
						>
							<div
								className={clsx(
									"px-2.5 py-1.5 rounded-lg",
									msg.isAI
										? "bg-accent/10 border border-accent/20"
										: msg.isUser
											? "bg-sidebar-selected/60"
											: "bg-app-box/60",
									msg.unread && "ring-1 ring-accent/50",
								)}
							>
								{!msg.isUser && (
									<div
										className={clsx(
											"text-[10px] font-semibold mb-0.5",
											msg.isAI
												? "text-accent"
												: "text-sidebar-inkDull",
										)}
									>
										{msg.sender}
									</div>
								)}
								<p className="text-xs text-sidebar-ink leading-relaxed">
									{msg.content}
								</p>
							</div>
							<span className="text-[10px] text-sidebar-inkDull mt-0.5 px-1">
								{msg.time}
							</span>
						</div>
					</div>
				))}
			</div>

			{/* Input */}
			<div className="border-t border-sidebar-line p-2 space-y-2">
				<div className="flex items-end gap-1.5">
					<button
						className="p-1.5 rounded-lg hover:bg-sidebar-selected transition-colors text-sidebar-inkDull hover:text-sidebar-ink"
						title="Attach file"
					>
						<Paperclip size={4} weight="bold" />
					</button>

					<div className="flex-1 flex items-center gap-1.5 bg-app-box border border-app-line rounded-lg px-2 py-1.5">
						<input
							type="text"
							value={message}
							onChange={(e) => setMessage(e.target.value)}
							placeholder="Type a message..."
							className="flex-1 bg-transparent text-xs text-sidebar-ink placeholder:text-sidebar-inkDull outline-none"
						/>
					</div>

					<button
						className="p-1.5 rounded-lg bg-accent hover:bg-accent/90 transition-colors text-white"
						title="Send message"
					>
						<PaperPlaneRight size={4} weight="bold" />
					</button>
				</div>

				<div className="flex gap-1">
					<button className="px-2 py-1 text-[10px] font-medium text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md transition-colors flex items-center gap-1">
						<Sparkle size={3} weight="bold" />
						Ask AI
					</button>
					<button className="px-2 py-1 text-[10px] font-medium text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md transition-colors">
						Share File
					</button>
					<button className="px-2 py-1 text-[10px] font-medium text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md transition-colors">
						Create Task
					</button>
				</div>
			</div>
		</div>
	);
}

function ActivityTab() {
	const activity = [
		{ action: "Synced to NAS", time: "2 min ago", device: "MacBook Pro" },
		{ action: "Uploaded to S3", time: "1 hour ago", device: "MacBook Pro" },
		{
			action: "Thumbnail generated",
			time: "2 hours ago",
			device: "MacBook Pro",
		},
		{ action: "Tagged as 'Travel'", time: "3 hours ago", device: "iPhone" },
		{ action: "Created", time: "Jan 15, 2025", device: "iPhone" },
	];

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				History of changes and sync operations
			</p>

			<div className="space-y-0.5">
				{activity.map((item, i) => (
					<div
						key={i}
						className="flex items-start gap-3 p-2 hover:bg-app-box/40 rounded-lg transition-colors"
					>
						<span className="text-sidebar-inkDull shrink-0 mt-0.5">
							<ClockCounterClockwise size={16} weight="bold" />
						</span>
						<div className="flex-1 min-w-0">
							<div className="text-xs text-sidebar-ink">
								{item.action}
							</div>
							<div className="text-[11px] text-sidebar-inkDull mt-0.5">
								{item.time} · {item.device}
							</div>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

function DetailsTab({ file }: { file: File }) {
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
								16,
							) + "..."
						}
						mono
					/>
					{file.content_identity.integrity_hash && (
						<InfoRow
							label="Integrity Hash"
							value={
								String(
									file.content_identity.integrity_hash,
								).slice(0, 16) + "..."
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
					value={String(file.id).slice(0, 8) + "..."}
					mono
				/>
				<InfoRow
					label="Content Kind"
					value={String(file.content_kind || "Unknown")}
				/>
				{file.extension && (
					<InfoRow label="Extension" value={String(file.extension)} />
				)}
			</Section>

			{/* System */}
			<Section title="System" icon={DotsThree}>
				<InfoRow label="Entry Kind" value={file.kind} />
				<InfoRow label="Local" value={file.is_local ? "Yes" : "No"} />
				<InfoRow
					label="Instances"
					value={String((file.alternate_paths?.length || 0) + 1)}
				/>
			</Section>
		</div>
	);
}
