'use client';

import { AnimatePresence } from 'framer-motion';
import { memo, useCallback, useState } from 'react';
import { SelectedVideo, Video } from '~/components/video';

const videos: {
	title: string;
	src: string;
	description: string;
}[] = [
	{
		title: 'Drag and Drop',
		src: '/videos/Spacedrive_DragAndDrop.webm',
		description: 'Easily drag and drop files or folders.'
	},
	{
		title: 'Tabs',
		src: '/videos/Spacedrive_Tabs.webm',
		description: 'Browse seamlessly with multiple tabs.'
	},
	{
		title: 'Quick Preview',
		src: '/videos/Spacedrive_QuickPreview.webm',
		description: 'Instantly preview files and object data.'
	}
];

const MemoizedVideo = memo(Video);

const MemoizedExplorer = memo(function Explorer() {
	const [selectedVideo, setSelectedVideo] = useState<null | string>(null);

	const handleVideoSelect = useCallback((src: string | null) => {
		setSelectedVideo(src);
	}, []);

	return (
		<>
			{selectedVideo ? (
				<AnimatePresence>
					<SelectedVideo src={selectedVideo} setSelectedVideo={setSelectedVideo} />
				</AnimatePresence>
			) : null}
			<div className="container mx-auto flex flex-col flex-wrap items-center gap-10 px-4">
				<h1 className="flex-1 self-start text-2xl font-semibold leading-8 md:text-3xl md:leading-10 lg:self-start">
					Explorer.{' '}
					<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
						{/* Some controlled line breaks here based on breakpoint to make sure the breaks looks nice always :) */}
						<br className="lg:hidden" />
						Browse and manage your data
						<br className="sm:hidden" /> like never before.
					</span>
				</h1>
				<div className="grid w-full grid-cols-1 gap-16 md:grid-cols-3 md:gap-4">
					{videos.map((video) => (
						<div key={video.src}>
							<MemoizedVideo
								setSelectedVideo={handleVideoSelect}
								layoutId={`video-${video.src}`}
								onClick={() => handleVideoSelect(video.src)}
								{...video}
							/>
							<h2 className="mt-5 text-lg font-bold text-white">{video.title}</h2>
							<p className="text-md text-ink-dull">{video.description}</p>
						</div>
					))}
				</div>
			</div>
		</>
	);
});

export { MemoizedExplorer as Explorer };
