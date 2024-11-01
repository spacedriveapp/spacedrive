import { getIcon, getIconByName, getLayeredIcon, IconTypes } from '@sd/assets/util';
import clsx from 'clsx';
import { forwardRef, Suspense, useMemo, type ImgHTMLAttributes } from 'react';
import { type ObjectKindKey } from '@sd/client';
import { useIsDark } from '~/hooks';

interface LayeredFileIconProps extends Omit<ImgHTMLAttributes<HTMLImageElement>, 'src'> {
	kind: ObjectKindKey;
	isDir: boolean;
	extension: string | null;
	customIcon: IconTypes | null;
}

const SUPPORTED_ICONS = ['Document', 'Code', 'Text', 'Config'];

const positionConfig: Record<string, string> = {
	Text: 'flex h-full w-full items-center justify-center',
	Code: 'flex h-full w-full items-center justify-center pt-[18px]',
	Config: 'flex h-full w-full items-center justify-center pt-[18px]'
};

const LayeredFileIcon = forwardRef<HTMLImageElement, LayeredFileIconProps>(
	({ kind, isDir, extension, customIcon, ...props }, ref) => {
		const isDark = useIsDark();

		const src = useMemo(
			() =>
				customIcon
					? getIconByName(customIcon, isDark)
					: getIcon(kind, isDark, extension, isDir),
			[customIcon, isDark, kind, extension, isDir]
		);

		const iconImg = <img ref={ref} src={src} {...props} alt={`${kind} icon`} />;

		if (SUPPORTED_ICONS.includes(kind) === false) {
			return iconImg;
		}

		const IconComponent = extension ? getLayeredIcon(kind, extension) : null;

		const positionClass =
			positionConfig[kind] || 'flex h-full w-full items-end justify-end pb-4 pr-2';

		return IconComponent == null ? (
			iconImg
		) : (
			<div className="relative">
				{iconImg}
				<div
					className={clsx('pointer-events-none absolute bottom-0 right-0', positionClass)}
				>
					<Suspense>
						<IconComponent viewBox="0 0 16 16" height="50%" width="50%" />
					</Suspense>
				</div>
			</div>
		);
	}
);

export default LayeredFileIcon;
