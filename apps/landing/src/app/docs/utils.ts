import { useParams } from 'next/navigation';

export function useDocsParams() {
	return useParams<{ slug?: string[] }>();
}
