import { useCallback, useState } from 'react';
import { Text, View } from 'react-native';
import { useDebouncedCallback } from 'use-debounce';
import { useLibraryMutation } from '@sd/client';
import { Object as SDObject } from '@sd/client';

type Props = {
	data: SDObject;
};

const Note = (props: Props) => {
	const [note, setNote] = useState(props.data.note || '');

	const { mutate: fileSetNote } = useLibraryMutation('files.setNote');

	const debounce = useDebouncedCallback(
		(note: string) =>
			fileSetNote({
				id: props.data.id,
				note
			}),
		2000
	);

	const debouncedNote = useCallback((note: string) => debounce(note), [props.data.id, fileSetNote]);

	return (
		<View>
			<Text>Note</Text>
		</View>
	);
};

export default Note;
