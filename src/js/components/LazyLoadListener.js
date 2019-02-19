
import React from 'react';
import * as helpers from '../helpers';

export default class LazyLoadListener extends React.Component{

	constructor(props){
		super(props);
		this.state = {
			listening: (this.props.loadKey ? true : false),
			loadKey: this.props.loadKey
		}

		this.handleScroll = helpers.throttle(this.handleScroll.bind(this), 50);
	}

	componentDidMount(){
		this.element = document.getElementById('main');
		this.element.addEventListener("scroll", this.handleScroll, false);
	}

	componentWillUnmount(){
		this.element.removeEventListener("scroll", this.handleScroll, false);
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.loadKey && nextProps.loadKey !== this.state.loadKey){
			this.setState({
				loadKey: nextProps.loadKey,
				listening: true
			});
		}
	}

	handleScroll(e){
		if (this.state.listening){

			var window_height = Math.max(document.documentElement.clientHeight, window.innerHeight || 0);

			// At, or half a screen from bottom of the page
		    if (this.element.scrollTop > (this.element.scrollHeight - this.element.offsetHeight - (window_height / 2))){

				// Immediately stop listening to avoid duplicating pagination requests
				this.setState(
					{listening: false},
					() => {
						console.info('Loading more: '+this.props.loadKey);
						this.props.loadMore();
					}
				);

		    }
		}
	}

	render(){
		return (
			<div className={"lazy-loader body-loader"+(this.props.showLoader ? ' loading' : '')}>
				{this.props.showLoader ? <div className="loader"></div> : null}
			</div>
		)
	}
}