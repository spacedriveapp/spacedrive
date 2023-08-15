import { IconMapping } from "./IconMapping";

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
	const IconComponent = ({ extension }: { extension: keyof typeof IconMapping }) => {
		return IconMapping[extension];
	};
	return (
		<div className="relative">
			<img
				src={src}
				onLoad={onLoad}
				onError={onError}
				decoding={size ? 'async' : 'sync'}
				draggable={false}
			/>
			{IconMapping.hasOwnProperty(extension) && (
				<div className="absolute bottom-0 right-0 flex h-full w-full items-end justify-end pb-4 pr-2">
					<IconComponent extension={extension as keyof typeof IconMapping} />
				</div>
			)}
		</div>
	);
};

export default LayeredFileIcon;
