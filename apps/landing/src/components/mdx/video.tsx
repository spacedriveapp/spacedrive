import React from 'react';

const Video = ({ url }: { url: string }) => {
	return (
		<div style={{ maxWidth: '100%' }}>
			<video
				loop
				autoPlay
				muted
				style={{ width: '100%', height: 'auto', borderRadius: 10, overflow: 'hidden' }}
			>
				<source src={url} type="video/mp4" />
				Your browser does not support the video tag.
			</video>
		</div>
	);
};

export default Video;
