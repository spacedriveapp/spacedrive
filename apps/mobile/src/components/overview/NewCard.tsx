import { Text, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { tw, twStyle } from '~/lib/tailwind';

import { Icon, IconName } from '../icons/Icon';
import Fade from '../layout/Fade';
import { Button } from '../primitive/Button';

type NewCardProps =
	| {
			icons: IconName[];
			text: string;
			style?: ClassInput;
			button?: () => JSX.Element;
			buttonText?: never;
			buttonHandler?: never;
	  }
	| {
			icons: IconName[];
			text: string;
			style?: ClassInput;
			buttonText: string;
			buttonHandler: () => void;
			button?: never;
	  };

export default function NewCard({
	icons,
	text,
	buttonText,
	buttonHandler,
	button,
	style
}: NewCardProps) {
	return (
		<View
			style={twStyle(
				'flex w-[280px] shrink-0 flex-col justify-between rounded border border-dashed border-app-lightborder p-4',
				style
			)}
		>
			<View style={tw`flex flex-row items-start justify-between`}>
				<Fade height={'100%'} width={70} color="black">
					<View style={twStyle(`flex flex-row`)}>
						{icons.map((iconName, index) => (
							<View key={index}>
								<Icon size={60} name={iconName} />
							</View>
						))}
					</View>
				</Fade>
			</View>
			<Text style={tw`text-sm text-ink-dull`}>{text}</Text>
			{button ? (
				button()
			) : (
				<Button variant="transparent" onPress={buttonHandler} disabled={!buttonText}>
					<Text style={tw`text-sm font-bold text-ink-dull`}>
						{' '}
						{buttonText ? buttonText : 'Coming Soon'}
					</Text>
				</Button>
			)}
		</View>
	);
}
