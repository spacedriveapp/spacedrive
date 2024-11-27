import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import { useMemo } from 'react';
import { humanizeSize } from '@sd/client';
import { Button, Card, tw } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

type LocationCardProps = {
	name: string;
	icon: string;
	totalSpace: string | number[];
	color: string;
	connectionType: 'lan' | 'p2p' | 'cloud' | null;
};

const Pill = tw.div`px-1.5 py-[1px] rounded text-tiny font-medium text-ink-dull bg-app-box border border-app-line`;

const LocationCard = ({ icon, name, connectionType, ...stats }: LocationCardProps) => {
	const { totalSpace } = useMemo(() => {
		return {
			totalSpace: humanizeSize(stats.totalSpace)
		};
	}, [stats]);
	const { t } = useLocale();

	return (
		<Card className="flex w-[280px] shrink-0 flex-col bg-app-box/50 !p-0">
			<div className="flex flex-row items-center gap-5 p-4 px-6">
				<div className="flex flex-col overflow-hidden">
					<Icon className="-ml-1" name={icon as any} size={60} />
					<span className="truncate font-medium">{name}</span>
					<span className="mt-1 truncate text-tiny text-ink-faint">
						{totalSpace.value}
						{t(`size_${totalSpace.unit.toLowerCase()}`)}
					</span>
				</div>
			</div>
			<div className="flex h-10 flex-row items-center gap-1.5 border-t border-app-line px-2">
				<Pill className="uppercase">{connectionType || 'Local'}</Pill>
				<div className="grow" />
				<Button size="icon" variant="outline">
					<Ellipsis className="size-3 opacity-50" />
				</Button>
			</div>
		</Card>
	);
};

export default LocationCard;
