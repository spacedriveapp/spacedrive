import { IconMapping } from './IconMapping';

const LayeredFileIcon = ({
	src,
	extension,
	size,
	onLoad,
	onError,
	...props
}: {
	src: string;
	extension: string;
	size: string;
	onLoad: any;
	onError: any;
	props: any;
}) => {
	const IconComponent = IconMapping[extension];
	return (
		<div className="relative">
			<img
				src={src}
				onLoad={onLoad}
				onError={onError}
				decoding={size ? 'async' : 'sync'}
				draggable={false}
			/>
			{IconComponent !== undefined && (
				<div className="absolute bottom-0 right-0 flex h-full w-full items-end justify-end pb-4 pr-2">
					<IconComponent viewBox="0 0 16 16" height="40%" width="40%" />
				</div>
			)}
		</div>
	);
};

export default LayeredFileIcon;
