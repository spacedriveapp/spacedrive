import { type ImgHTMLAttributes } from 'react';
import { type ObjectKindKey } from '@sd/client';
import { getLayeredIcon } from '@sd/assets/util';

interface LayeredFileIconProps extends ImgHTMLAttributes<HTMLImageElement> {
	kind: ObjectKindKey;
	extension: string | null;
}

const LayeredFileIcon = ({ kind, extension, ...props }: LayeredFileIconProps) => {
	const iconImg = <img {...props} />;
	const IconComponent = extension ? getLayeredIcon(kind, extension) : null;

	return IconComponent == null ? (
		iconImg
	) : (
		<div className="relative">
			{iconImg}
			<div className="absolute bottom-0 right-0 flex h-full w-full items-end justify-end pb-4 pr-2">
				<IconComponent viewBox="0 0 16 16" height="40%" width="40%" />
			</div>
		</div>
	);
};

export default LayeredFileIcon;
