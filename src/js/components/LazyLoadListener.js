
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

export default class LazyLoadListener extends React.Component{

	constructor(props) {
		super(props);
		this.state = { loading: false }
		this.handleScroll = this.handleScroll.bind(this);
	}

	componentWillMount(){
		window.addEventListener("scroll", this.handleScroll, false);
	}

	componentWillUnmount(){
		window.removeEventListener("scroll", this.handleScroll, false);
	}

	handleScroll(e){
	    if( (window.innerHeight + window.scrollY) >= (document.body.offsetHeight - 80) ){
	    	if( !this.state.loading && this.props.enabled ){
				this.setState({ loading: true })
				this.props.loadMore();
			}
	    }else if( this.state.loading ){
			this.setState({ loading: false })
	    }
	}

	render(){
		return (
			<div className="lazy-loader body-loader">
				{this.state.loading ? <div className="loader"></div> : null}
			</div>
		)
	}
}