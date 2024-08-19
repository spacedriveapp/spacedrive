import { ArrowSquareOut, Trash } from '@phosphor-icons/react';
import clsx from 'clsx';
import { PropsWithChildren } from 'react';
import { Button, toast, tw } from '@sd/ui';
import { Icon, IconName } from '~/components';
import { useLocale, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { useExplorerDroppable } from '../../../../Explorer/useExplorerDroppable';
import { useExplorerSearchParams } from '../../../../Explorer/util';
import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import { SeeMore } from '../../SidebarLayout/SeeMore';

const Name = tw.span`truncate`;

const OpenToButton = ({ className }: { className?: string; what_is_opening?: string }) => (
	<Button className={clsx('absolute right-[2px] !p-[5px]', className)} variant="subtle">
		<ArrowSquareOut size={18} className="size-3 opacity-70" />
	</Button>
);

export default function ToolsSection() {
	const platform = usePlatform();

	const { t } = useLocale();

	const os = useOperatingSystem();
	return (
		<Section name={t('tools')}>
			<SeeMore>
				{platform.openTrashInOsExplorer && (
					<button
						// eslint-disable-next-line tailwindcss/migration-from-tailwind-2
						className={`max-w relative flex w-full grow flex-row items-center gap-0.5 truncate rounded border border-transparent ${os === 'macOS' ? 'bg-opacity-90' : ''} px-2 py-1 text-sm font-medium text-sidebar-inkDull outline-none ring-0 ring-inset ring-transparent ring-offset-0 focus:ring-1 focus:ring-accent focus:ring-offset-0`}
						onClick={() => {
							platform.openTrashInOsExplorer?.();
							toast.info(t('opening_trash'));
						}}
					>
						<Trash size={18} className="mr-1" />
						<Name>{t('trash')}</Name>
						<OpenToButton />
					</button>
				)}
			</SeeMore>
		</Section>
	);
}

const EphemeralLocation = ({
	children,
	path,
	navigateTo
}: PropsWithChildren<{ path: string; navigateTo: string }>) => {
	const [{ path: ephemeralPath }] = useExplorerSearchParams();

	const { isDroppable, className, setDroppableRef } = useExplorerDroppable({
		id: `sidebar-ephemeral-location-${path}`,
		allow: ['Path', 'NonIndexedPath', 'Object'],
		data: { type: 'location', path },
		disabled: navigateTo.startsWith('location/') || ephemeralPath === path,
		navigateTo: navigateTo
	});

	return (
		<SidebarLink
			ref={setDroppableRef}
			to={navigateTo}
			className={clsx(
				'border',
				isDroppable ? 'border-accent' : 'border-transparent',
				className
			)}
		>
			{children}
		</SidebarLink>
	);
};
