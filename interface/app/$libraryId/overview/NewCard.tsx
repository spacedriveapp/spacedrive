// import { X } from '@phosphor-icons/react';
import { Icon, IconName } from '~/components';

type NewCardProps =
	| {
			icons: IconName[];
			text: string;
			button?: () => JSX.Element;
			buttonText?: never;
			buttonHandler?: never;
	  }
	| {
			icons: IconName[];
			text: string;
			buttonText: string;
			buttonHandler: () => void;
			button?: never;
	  };

const maskImage = `linear-gradient(90deg, transparent 0.1%, rgba(0, 0, 0, 1), rgba(0, 0, 0, 1) 35%, transparent 99%)`;

export default function NewCard({ icons, text, buttonText, buttonHandler, button }: NewCardProps) {
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
			</div>
			<span className="text-sm text-ink-dull">{text}</span>
			{button ? (
				button()
			) : (
				<button
					onClick={buttonHandler}
					disabled={!buttonText}
					className="text-sm font-medium text-ink-dull"
				>
					{buttonText ? buttonText : 'Coming Soon'}
				</button>
			)}
		</div>
	);
}
