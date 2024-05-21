import { Outlet } from 'react-router';

export const Component = () => (
	<div className="custom-scroll page-scroll relative flex size-full max-h-screen grow-0 pt-6">
		<div className="flex w-full max-w-4xl flex-col space-y-6 px-12 pb-5 pt-2">
			<Outlet />
			<div className="block h-4 shrink-0" />
		</div>
	</div>
);
