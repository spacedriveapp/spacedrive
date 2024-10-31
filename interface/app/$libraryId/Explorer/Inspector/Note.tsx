import { useEffect, useRef } from 'react';
import { useDebouncedCallback } from 'use-debounce';
import { useSnapshot } from 'valtio';
import { Object as SDObject, useLibraryMutation } from '@sd/client';
import { Divider, TextArea } from '@sd/ui';
import { useLocale } from '~/hooks';

import { MetaContainer, MetaTitle } from '../Inspector';
import { noteCacheStore } from './store';

interface Props {
	data: SDObject;
}

export default function Note({ data }: Props) {
	const setNote = useLibraryMutation('files.setNote');

	const flush = useRef<() => void>();
	const debouncedSetNote = useDebouncedCallback((note: string) => {
		setNote.mutate({
			id: data.id,
			note
		});
	}, 500);

	// Flush debounced note change when component unmounts
	flush.current = debouncedSetNote.flush;

	// Cleanup on unmount
	useEffect(() => () => flush.current?.(), []);

	// Use Valtio snapshot to manage cached notes
	const noteSnapshot = useSnapshot(noteCacheStore);

	// Prioritize cached value unless backend value is different, then update
	useEffect(() => {
		const cachedNote = noteCacheStore[data.id];
		if (cachedNote === undefined || cachedNote === data.note) {
			// If no cached value or cached value equals backend value, update store with backend value
			noteCacheStore[data.id] = data.note ?? undefined;
		}
	}, [data]);

	const { t } = useLocale();

	return (
		<>
			<Divider />
			<MetaContainer>
				<MetaTitle>{t('note')}</MetaTitle>
				<TextArea
					className="mb-1 mt-2 !py-2 text-xs leading-snug"
					value={noteSnapshot[data.id] ?? ''}
					onChange={(e) => {
						noteCacheStore[data.id] = e.target.value;
						debouncedSetNote(e.target.value);
					}}
				/>
			</MetaContainer>
		</>
	);
}
