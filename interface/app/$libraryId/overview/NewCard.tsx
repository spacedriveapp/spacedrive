import { X } from '@phosphor-icons/react';
import { Button } from '@sd/ui';
import { Icon, IconName } from '~/components';

interface NewCardProps {
	icons: IconName[];
	text: string;
	buttonText?: string;
}

const maskImage = `linear-gradient(90deg, transparent 0.1%, rgba(0, 0, 0, 1), rgba(0, 0, 0, 1) 35%, transparent 99%)`;

const NewCard = ({ icons, text, buttonText }: NewCardProps) => {
	return (
		<div className="flex h-[170px] w-[280px] shrink-0 flex-col justify-between rounded border border-dashed border-app-line p-4">
			<div className="flex flex-row items-start justify-between">
				<div
					className="flex flex-row"
					style={{
						WebkitMaskImage: maskImage,
						maskImage
					}}
				>
					{icons.map((iconName, index) => (
						<div key={index}>
							<Icon size={60} name={iconName} />
						</div>
					))}
				</div>
				<Button size="icon" variant="outline">
					<X weight="bold" className="h-3 w-3 opacity-50" />
				</Button>
			</div>
			<span className="text-sm text-ink-dull">{text}</span>
			<Button disabled={!buttonText} variant="outline">
				{buttonText ? buttonText : 'Coming Soon'}
			</Button>
		</div>
	);
};

export default NewCard;
