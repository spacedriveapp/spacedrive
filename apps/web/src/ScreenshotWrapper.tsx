import * as htmlToImage from 'html-to-image';
import React, { useEffect, useRef } from 'react';

const ScreenshotWrapper = ({
	showControls,
	children
}: {
	showControls: boolean;
	children: React.ReactNode;
}) => {
	const domEl = useRef(null);

	const downloadImage = async () => {
		const style = document.createElement('style');
		style.innerHTML = `
            ::-webkit-scrollbar {
                display: none;
            }
            body, .no-scrollbar, .custom-scroll {
                overflow: hidden !important;
                -ms-overflow-style: none;
                scrollbar-width: none;
            }
        `;
		document.head.appendChild(style);

		if (!domEl.current) return;
		const dataUrl = await htmlToImage.toPng(domEl.current);

		document.head.removeChild(style);

		const link = document.createElement('a');
		link.download = 'test.png';
		link.href = dataUrl;
		link.click();
	};

	useEffect(() => {
		if (showControls) {
			window.document.body.style.backgroundColor = 'black';
			window.addEventListener('keyup', (e) => {
				if (e.key === 'k') {
					downloadImage();
				}
			});
			return () => window.removeEventListener('keyup', downloadImage);
		}
	}, [showControls]);

	return (
		<div
			ref={showControls ? domEl : null}
			style={
				showControls
					? {
							width: '1278px',
							height: '626px',
							margin: '0 auto',
							position: 'relative',
							overflow: 'hidden'
						}
					: {}
			}
		>
			{children}
		</div>
	);
};

export default ScreenshotWrapper;
