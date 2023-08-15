import * as Icons from '@sd/assets/icons/ext';

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
			<div className='flex absolute bottom-0 right-0 h-full w-full items-end justify-end pb-4 pr-2'>
				<Icons.go viewBox='0 0 16 16' height='40%' width='40%' />
			</div>
		</div>
	)
}

export default LayeredFileIcon
