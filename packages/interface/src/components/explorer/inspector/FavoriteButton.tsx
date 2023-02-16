import { Heart } from 'phosphor-react';
import { useEffect, useState } from 'react';
import { useLibraryMutation } from '@sd/client';
import { Object as SDObject } from '@sd/client';
import { Button } from '@sd/ui';

interface Props {
	data: SDObject;
}

export default function FavoriteButton(props: Props) {
	const [favorite, setFavorite] = useState(false);

	useEffect(() => {
		setFavorite(!!props.data?.favorite);
	}, [props.data]);

	const { mutate: fileToggleFavorite, isLoading: isFavoriteLoading } = useLibraryMutation(
		'files.setFavorite'
		// {
		// 	onError: () => setFavorite(!!props.data?.favorite)
		// }
	);

	const toggleFavorite = () => {
		if (!isFavoriteLoading) {
			fileToggleFavorite({ id: props.data.id, favorite: !favorite });
			setFavorite(!favorite);
		}
	};

	return (
		<Button onClick={toggleFavorite} size="icon">
			<Heart weight={favorite ? 'fill' : 'regular'} className="h-[18px] w-[18px]" />
		</Button>
	);
}
