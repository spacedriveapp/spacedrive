// Loosen the type definition of the `useSnapshot` hook
import 'valtio';

declare module 'valtio' {
	function useSnapshot<T extends object>(p: T): T;
}
