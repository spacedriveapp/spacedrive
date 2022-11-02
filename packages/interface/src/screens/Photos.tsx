export default function PhotosScreen() {
	return (
		<div className="flex flex-col w-full h-screen p-5 custom-scroll page-scroll app-background">
			<div className="flex flex-col space-y-5 pb-7">
				<p className="px-5 py-3 mb-3 text-sm border rounded-md shadow-sm border-app-line bg-app-box ">
					<b>Note: </b>This is a pre-alpha build of Spacedrive, many features are yet to be
					functional.
				</p>
				{/* <Spline
					style={{ height: 500 }}
					height={500}
					className="rounded-md shadow-sm pointer-events-auto"
					scene="https://prod.spline.design/KUmO4nOh8IizEiCx/scene.splinecode"
				/> */}
			</div>
		</div>
	);
}
