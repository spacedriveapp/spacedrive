'use client';

import Image, { ImageProps } from 'next/image';
import React, { useEffect, useState } from 'react';

interface CyclingImageProps extends Omit<ImageProps, 'src'> {
	images: string[];
}

const CyclingImage: React.FC<CyclingImageProps> = ({ images, width, height, ...imgProps }) => {
	const [currentIndex, setCurrentIndex] = useState(0);
	const [isHovering, setIsHovering] = useState(false);

	useEffect(() => {
		let timeoutId: number;
		if (isHovering && images.length > 1) {
			const nextIndex = (currentIndex + 1) % images.length;
			const img = new window.Image();
			img.src = images[nextIndex];
			img.onload = () => {
				timeoutId = window.setTimeout(() => setCurrentIndex(nextIndex), 1500);
			};
		}
		return () => window.clearTimeout(timeoutId);
	}, [isHovering, currentIndex, images]);

	return (
		<div onMouseEnter={() => setIsHovering(true)} onMouseLeave={() => setIsHovering(false)}>
			{images.map((src, index) => (
				<div
					key={src}
					style={{
						display: index === currentIndex ? 'block' : 'none',
						position: 'relative',
						width: '100%',
						maxWidth: width,
						height
					}}
				>
					<Image {...imgProps} src={src} alt="" width={width} height={height} />
				</div>
			))}
		</div>
	);
};

export default CyclingImage;
