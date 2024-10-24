import { Heart } from '@phosphor-icons/react';
import { useEffect, useState } from 'react';
import { Object as SDObject, useLibraryMutation } from '@sd/client';
import { Button } from '@sd/ui';

interface Props {
	data: SDObject;
}

export default function FavoriteButton(props: Props) {
	const [favorite, setFavorite] = useState(false);

	useEffect(() => {
		setFavorite(!!props.data?.favorite);
	}, [props.data]);

	const { mutate: fileToggleFavorite, isPending: isFavoriteLoading } = useLibraryMutation(
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
			<Heart weight={favorite ? 'fill' : 'regular'} className="size-[18px]" />
		</Button>
	);
}
