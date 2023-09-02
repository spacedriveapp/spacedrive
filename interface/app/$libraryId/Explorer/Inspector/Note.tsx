import { useEffect, useState } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { Object as SDObject, useLibraryMutation } from '@sd/client';
import { Divider, TextArea } from '@sd/ui';
import { MetaContainer, MetaTitle } from '../Inspector';
import { useExplorerStore } from '../store';

interface Props {
	data: SDObject;
}

export default function Note(props: Props) {
	const setNote = useLibraryMutation('files.setNote');

	const explorerStore = useExplorerStore();

	const debouncedSetNote = useDebouncedCallback((note: string) => {
		setNote.mutate({
			id: props.data.id,
			note
		});
	}, 500);

	// Force update when component unmounts
	useEffect(() => () => debouncedSetNote.flush(), []);

	const [cachedNote, setCachedNote] = useState(props.data.note);

	return (
		<>
			<Divider />
			<MetaContainer>
				<MetaTitle>Note</MetaTitle>
				<TextArea
					className="mb-1 mt-2 !py-2 text-xs leading-snug"
					value={cachedNote ?? ''}
					onChange={(e) => {
						setCachedNote(e.target.value);
						debouncedSetNote(e.target.value);
					}}
				/>
			</MetaContainer>
		</>
	);
}
