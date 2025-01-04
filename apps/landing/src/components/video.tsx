import clsx from 'clsx';
import { motion, MotionProps } from 'framer-motion';
import { ComponentProps, useEffect, useRef } from 'react';
import { useClickOutside } from '~/hooks/useClickOutside';

type Props = ComponentProps<'video'> &
	MotionProps & {
		containerClassName?: string;
		setSelectedVideo: (src: string | null) => void;
	};

const Video = ({ containerClassName, setSelectedVideo, ...rest }: Props) => {
	const videoRef = useRef<HTMLVideoElement>(null);
	const observerRef = useRef<IntersectionObserver | null>(null);

	useClickOutside(videoRef, () => setSelectedVideo(null));

	useEffect(() => {
		const videoElement = videoRef.current;
		if (!videoElement) return;

		observerRef.current = new IntersectionObserver(
			(entries) => {
				entries.forEach((entry) => {
					if (entry.isIntersecting) {
						videoElement.play().catch(() => {
							// Autoplay might be blocked by browser
							console.debug('Video autoplay blocked');
						});
					} else {
						videoElement.pause();
					}
				});
			},
			{ threshold: 0.2 } // 20% visibility threshold
		);

		observerRef.current.observe(videoElement);

		return () => {
			if (observerRef.current) {
				observerRef.current.disconnect();
			}
		};
	}, []);

	return (
		<div className={clsx(containerClassName, 'relative')}>
			<motion.video
				style={{
					borderRadius: '12px',
					width: '100%',
					height: '100%'
				}}
				ref={videoRef}
				layoutId={`video-${rest.src}`}
				whileHover={{
					scale: 1.05
				}}
				whileTap={{ scale: 1 }}
				autoPlay
				loop
				muted
				playsInline
				className="size-full cursor-pointer rounded-xl object-cover"
				{...rest}
			/>
		</div>
	);
};

const SelectedVideo = ({
	src,
	setSelectedVideo
}: {
	src: string;
	setSelectedVideo: (src: string | null) => void;
}) => {
	const videoRef = useRef<HTMLVideoElement>(null);
	useClickOutside(videoRef, () => setSelectedVideo(null));

	return (
		<>
			<motion.div
				initial={{ opacity: 0 }}
				animate={{ opacity: 1 }}
				exit={{ opacity: 0 }}
				className="bg-opacity/50 fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-md"
			/>
			<motion.div
				className="fixed inset-0 z-[60] mx-auto flex w-full max-w-[900px] items-center justify-center p-5 md:p-0"
				initial={{ scale: 0.8, opacity: 0 }}
				animate={{ scale: 1, opacity: 1 }}
				exit={{ scale: 0.8, opacity: 0 }}
				transition={{ duration: 0.2, ease: [0.23, 1, 0.32, 1] }}
			>
				<div className="w-full overflow-hidden rounded-xl">
					<motion.video
						src={src}
						autoPlay
						loop
						muted
						playsInline
						layoutId={`video-${src}`}
						transition={{ duration: 0.3, ease: 'easeInOut' }}
						className="h-full w-full object-contain"
					/>
				</div>
			</motion.div>
		</>
	);
};

export { SelectedVideo, Video };
