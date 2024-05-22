import { useEffect, useRef, useState } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { Object as SDObject, useLibraryMutation } from '@sd/client';
import { Divider, TextArea } from '@sd/ui';
import { useLocale } from '~/hooks';

import { MetaContainer, MetaTitle } from '../Inspector';

interface Props {
	data: SDObject;
}

export default function Note(props: Props) {
	const setNote = useLibraryMutation('files.setNote');

	const flush = useRef<() => void>();
	const debouncedSetNote = useDebouncedCallback((note: string) => {
		setNote.mutate({
			id: props.data.id,
			note
		});
	}, 500);

	// Don't need to wrap in a arrow func because flush is not a method
	flush.current = debouncedSetNote.flush;

	// Force update when component unmounts
	useEffect(() => () => flush.current?.(), []);

	const [cachedNote, setCachedNote] = useState(props.data.note);
	const { t } = useLocale();

	return (
		<>
			<Divider />
			<MetaContainer>
				<MetaTitle>{t('note')}</MetaTitle>
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
