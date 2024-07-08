import * as Haptics from 'expo-haptics';
import { Heart } from 'phosphor-react-native';
import { useState } from 'react';
import { Pressable, PressableProps } from 'react-native';
import { Object as SDObject, useLibraryMutation } from '@sd/client';

type Props = {
	data: SDObject;
	style: PressableProps['style'];
};

const FavoriteButton = (props: Props) => {
	const [favorite, setFavorite] = useState(props.data.favorite);

	const { mutate: toggleFavorite, isLoading } = useLibraryMutation('files.setFavorite', {
		onSuccess: () => {
			// TODO: Invalidate search queries
			setFavorite(!favorite);
			Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
		}
	});

	return (
		<Pressable
			disabled={isLoading}
			onPress={() => toggleFavorite({ id: props.data.id, favorite: !favorite })}
			style={props.style}
		>
			<Heart color="white" size={22} weight={favorite ? 'fill' : 'regular'} />
		</Pressable>
	);
};

export default FavoriteButton;
