import clsx from 'clsx';
import { motion, MotionProps } from 'framer-motion';
import { ComponentProps, useRef } from 'react';
import { useClickOutside } from '~/hooks/useClickOutside';

type Props = ComponentProps<'video'> &
	MotionProps & {
		containerClassName?: string;
		setSelectedVideo: (src: string | null) => void;
	};

const Video = ({ containerClassName, setSelectedVideo, ...rest }: Props) => {
	const videoRef = useRef<HTMLVideoElement>(null);
	useClickOutside(videoRef, () => setSelectedVideo(null));
	return (
		<div className={clsx(containerClassName)}>
			<motion.video
				style={{
					borderRadius: 12 // for framer-motion
				}}
				ref={videoRef}
				whileHover={{
					scale: 1.05
				}}
				whileTap={{ scale: 1 }}
				autoPlay
				loop
				muted
				playsInline
				className="size-full cursor-pointer object-cover"
				{...rest}
			/>
		</div>
	);
};

const SelectedVideo = ({ src }: { src: string }) => {
	return (
		<>
			<motion.div
				initial={{ opacity: 0 }}
				animate={{ opacity: 1 }}
				exit={{ opacity: 0 }}
				className="bg-opacity/50 fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-md"
			/>
			<div className="fixed inset-0 z-[60] mx-auto flex w-full max-w-[900px] items-center justify-center p-5 md:p-0">
				<motion.video
					src={src}
					style={{
						borderRadius: 12
					}}
					transition={{ duration: 0.3, ease: 'easeInOut' }}
					autoPlay
					layoutId={`video-${src}`}
					loop
					muted
					playsInline
					className="object-fill"
				/>
			</div>
		</>
	);
};

export { SelectedVideo, Video };
