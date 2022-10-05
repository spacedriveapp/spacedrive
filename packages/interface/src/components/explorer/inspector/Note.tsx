import { useLibraryMutation } from '@sd/client';
import { Object as SDObject } from '@sd/client';
import { TextArea } from '@sd/ui';
import debounce from 'lodash/debounce';
import { useCallback, useState } from 'react';

import { Divider } from './Divider';
import { MetaItem } from './MetaItem';

interface Props {
	data: SDObject;
}

export default function Note(props: Props) {
	// notes are cached in a store by their file id
	// this is so we can ensure every note has been sent to Rust even
	// when quickly navigating files, which cancels update function
	const [note, setNote] = useState(props.data.note || '');

	const { mutate: fileSetNote } = useLibraryMutation('files.setNote');

	const debouncedNote = useCallback(
		(note: string) =>
			debounce(
				() =>
					fileSetNote({
						id: props.data.id,
						note
					}),
				2000
			),
		[props.data.id, fileSetNote]
	);

	// when input is updated, cache note
	function handleNoteUpdate(e: React.ChangeEvent<HTMLTextAreaElement>) {
		if (e.target.value !== note) {
			setNote(e.target.value);
			debouncedNote(e.target.value);
		}
	}

	return (
		<>
			<Divider />
			<MetaItem
				title="Note"
				value={
					<TextArea
						className="mt-2 text-xs leading-snug !py-2"
						value={note || ''}
						onChange={handleNoteUpdate}
					/>
				}
			/>
		</>
	);
}
