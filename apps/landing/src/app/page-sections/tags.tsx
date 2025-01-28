'use client';

import { AnimatePresence } from 'framer-motion';
import React, { memo, useCallback, useState } from 'react';
import { SelectedVideo, Video } from '~/components/video';

const videos: {
	title: string;
	src: string;
	description: string;
}[] = [
	{
		title: 'Tag Assignment mode',
		src: '/videos/Spacedrive_tagmode.webm',
		description: 'Assign tags to files and folders quickly and easily'
	},
	{
		title: 'Contextual Tagging',
		src: '/videos/Spacedrive_tags.webm',
		description: 'Tag files and folders directly from the right-click menu'
	}
];

const MemoizedVideo = memo(Video);

const MemoizedTags = memo(function Tags() {
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
			<div className="container mx-auto flex flex-col flex-wrap items-center gap-10 p-4">
				<h1 className="flex-1 self-start text-2xl font-semibold leading-8 md:text-3xl md:leading-10 lg:self-start">
					Multiple ways to set tags.{' '}
					<span className="bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text text-transparent">
						{/* Some controlled line breaks here based on breakpoint to make sure the breaks looks nice always :) */}
						<br className="lg:hidden" />
						Quickly organize your files.
					</span>
				</h1>
				<div className="grid w-full grid-cols-1 gap-16 md:grid-cols-2 md:gap-6">
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

export { MemoizedTags as Tags };
