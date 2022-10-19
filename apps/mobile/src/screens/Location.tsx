import { useEffect } from 'react';
import Explorer from '~/components/explorer/Explorer';
import { SharedScreenProps } from '~/navigation/SharedScreens';
import { getExplorerStore } from '~/stores/explorerStore';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id, path } = route.params;

	useEffect(() => {
		// Not sure why we do this.
		getExplorerStore().locationId = id;
	}, [id]);

	return <Explorer locationId={id} path={path} />;
}
