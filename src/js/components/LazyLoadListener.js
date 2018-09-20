
import React, { PropTypes } from 'react';
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

	componentWillMount(){
		window.addEventListener("scroll", this.handleScroll, false);
	}

	componentWillUnmount(){
		window.removeEventListener("scroll", this.handleScroll, false);
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

			// At, or nearly at the bottom of the page
		    if ((window.innerHeight + window.scrollY) >= (document.body.offsetHeight - 80)){

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