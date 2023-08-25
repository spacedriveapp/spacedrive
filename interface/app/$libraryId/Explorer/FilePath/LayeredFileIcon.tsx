import { getLayeredIcon } from '@sd/assets/util';
import { type ImgHTMLAttributes } from 'react';
import { type ObjectKindKey } from '@sd/client';

interface LayeredFileIconProps extends ImgHTMLAttributes<HTMLImageElement> {
	kind: ObjectKindKey;
	extension: string | null;
}

const LayeredFileIcon = ({ kind, extension, ...props }: LayeredFileIconProps) => {
	const iconImg = <img {...props} />;

	if (kind !== 'Document' && kind !== 'Code' && kind !== 'Text' && kind !== 'Config') {
		return iconImg;
	}

	const IconComponent = extension ? getLayeredIcon(kind, extension) : null;

	const positionConfig: Record<string, string> = {
		Text: 'flex h-full w-full items-center justify-center'
		// Add more kinds here as needed
	};

	const positionClass =
		positionConfig[kind] || 'flex h-full w-full items-end justify-end pb-4 pr-2';

	return IconComponent == null ? (
		iconImg
	) : (
		<div className="relative">
			{iconImg}
			<div className={`absolute bottom-0 right-0 ${positionClass}`}>
				<IconComponent viewBox="0 0 16 16" height="40%" width="40%" />
			</div>
		</div>
	);
};

export default LayeredFileIcon;
