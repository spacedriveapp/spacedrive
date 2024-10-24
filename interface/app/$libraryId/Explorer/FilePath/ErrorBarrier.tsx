import React, { Component, ReactNode } from 'react';

interface ErrorBarrierProps {
	onError: (error: Error, info: React.ErrorInfo) => void;
	children: ReactNode;
}

interface ErrorBarrierState {
	hasError: boolean;
}

export class ErrorBarrier extends Component<ErrorBarrierProps, ErrorBarrierState> {
	constructor(props: ErrorBarrierProps) {
		super(props);
		this.state = { hasError: false };
	}

	static getDerivedStateFromError(error: Error) {
		// Update state so the next render will show the fallback UI.
		return { hasError: true };
	}

	componentDidCatch(error: Error, info: React.ErrorInfo) {
		// Call the onError function passed as a prop
		this.props.onError(error, info);
		// Reset the error state after calling onError
		Promise.resolve().then(() => this.setState({ hasError: false }));
	}

	render() {
		if (this.state.hasError) {
			// Render nothing since the parent component will handle the error
			return null;
		}

		return this.props.children;
	}
}

export default ErrorBarrier;
