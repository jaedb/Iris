
import React, { PropTypes } from 'react'

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
	    if( (window.innerHeight + window.scrollY) >= document.body.offsetHeight ){
	    	if( !this.state.loading ){
				this.setState({ loading: true })
				this.props.loadMore();
			}
	    }else if( this.state.loading ){
			this.setState({ loading: false })
	    }
	}

	render(){
		if( this.loading ){
			return <div className='lazy-loader loading'><div className="content">LOADING</div></div>
		}else{
			return <div className='lazy-loader'></div>
		}
	}
}