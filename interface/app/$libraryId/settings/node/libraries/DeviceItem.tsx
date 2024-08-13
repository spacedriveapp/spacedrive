import { Trash } from '@phosphor-icons/react';
import { humanizeSize } from '@sd/client';
import { Button, Card, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

interface DeviceItemProps {
	pub_id: Key | null | undefined;
	name: string;
	os: string;
	storage_size: number;
	created_at: string;
}

// unsure where to put pub_id/if this information is important for a user?
export default (props: DeviceItemProps) => {
	const { t } = useLocale();

	return (
		<Card className="flex items-center space-x-3 p-2">
			<Icon
				name={props.os == 'MacOS' || props.os == 'iOS' ? 'SilverBox' : 'Laptop'}
				alt="Device icon"
				size={24}
				className="mr-2"
			/>
			<div className="flex-1">
				<h4 className="text-sm font-semibold">{props.name}</h4>
				<p className="text-xs text-ink-dull">
					{props.os}, {`${t('added')}`} {new Date(props.created_at).toLocaleDateString()}
				</p>
				<p className="text-xs text-ink-dull"></p>
			</div>
			<div className="text-xs text-ink-dull">{`${humanizeSize(props.storage_size)}`}</div>
			<Button
				className="!p-1"
				variant="gray"
				onClick={() => {
					// Handle device-specific actions like delete, edit, etc.
				}}
			>
				<Tooltip label={t('Delete device')}>
					<Trash className="size-4" />
				</Tooltip>
			</Button>
		</Card>
	);
};
