import { useBridgeQuery } from '@sd/client';
import { Button, toast, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';

export default function DevicesSection() {
	const { data: node } = useBridgeQuery(['nodeState']);

	const { t } = useLocale();

	return (
		<Section name={t('devices')}>
			{node && (
				<SidebarLink className="group relative w-full" to={`node/${node.id}`} key={node.id}>
					<Icon name="Laptop" className="mr-1 size-5" />
					<span className="truncate">{node.name}</span>
				</SidebarLink>
			)}

			<Tooltip label={t('devices_coming_soon_tooltip')} position="right">
				<Button
					onClick={() => {
						toast.info(t('coming_soon'));
					}}
					variant="dotted"
					className="mt-1 w-full opacity-70"
				>
					{t('add_device')}
				</Button>
			</Tooltip>
		</Section>
	);
}
