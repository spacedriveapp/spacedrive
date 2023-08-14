import * as icons from '../../../../../packages/assets/icons/ext/bearded-icons/icons/index';

const LayeredFileIcon = ({src, size, onLoad, onError, ...props}: {src: string, size: string, onLoad: any, onError: any, props: any}) => {
	return (
		<div className='relative'>
			<img
				src={src}
				onLoad={onLoad}
				onError={onError}
				decoding={size ? 'async' : 'sync'}
				draggable={false}
			/>
			<div className='flex absolute top-0 left-0 h-full w-full items-center justify-center mt-3'>
				<svgMapping.rust viewBox='0 0 16 16' height='50%' width='50%' />
			</div>
		</div>
	)
}

export default LayeredFileIcon
