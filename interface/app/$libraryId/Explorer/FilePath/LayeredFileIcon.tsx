import * as Icons from '@sd/assets/icons/ext';

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
	const IconMapping = {
		rs: <Icons.rust viewBox="0 0 16 16" height="40%" width="40%" />,
		go: <Icons.go viewBox="0 0 16 16" height="40%" width="40%" />,
		html: <Icons.html viewBox="0 0 16 16" height="40%" width="40%" />,
		css: <Icons.css viewBox="0 0 16 16" height="40%" width="40%" />,
		scss: <Icons.scss viewBox="0 0 16 16" height="40%" width="40%" />,
		js: <Icons.js viewBox="0 0 16 16" height="40%" width="40%" />,
		jsx: <Icons.js viewBox="0 0 16 16" height="40%" width="40%" />,
		ts: <Icons.tsx viewBox="0 0 16 16" height="40%" width="40%" />,
		tsx: <Icons.tsx viewBox="0 0 16 16" height="40%" width="40%" />,
		vue: <Icons.vue viewBox="0 0 16 16" height="40%" width="40%" />,
		swift: <Icons.swift viewBox="0 0 16 16" height="40%" width="40%" />,
		php: <Icons.php viewBox="0 0 16 16" height="40%" width="40%" />,
		py: <Icons.python viewBox="0 0 16 16" height="40%" width="40%" />,
		rb: <Icons.ruby viewBox="0 0 16 16" height="40%" width="40%" />,
		sh: <Icons.shell viewBox="0 0 16 16" height="40%" width="40%" />,
	};
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
