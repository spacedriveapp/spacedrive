import { Outlet } from 'react-router';

export default () => (
	<div className="custom-scroll page-scroll relative flex h-full max-h-screen w-full grow-0">
		<div className="flex w-full max-w-4xl flex-col space-y-6 px-12 pt-2 pb-5">
			<Outlet />
			<div className="block h-20" />
		</div>
	</div>
);
