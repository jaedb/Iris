
import React from 'react'
import { Link } from 'react-router'

export default class URILink extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		var to = null;
		var uri = encodeURIComponent(this.props.uri);

		switch (this.props.type){
			
			case 'playlist':
				to = global.baseURL+'playlist/'+uri;
				break;
			
			case 'artist':
				to = global.baseURL+'artist/'+uri;
				break;

			case 'album':
				to = global.baseURL+'album/'+uri;
				break;

			case 'track':
				to = global.baseURL+'track/'+uri;
				break;

			case 'user':
				to = global.baseURL+'user/'+uri;
				break;

			case 'recommendations':
				to = global.baseURL+'discover/recommendations/'+uri;
				break;

			default:
				return null;
		}

		return (
			<Link 
				className={this.props.className ? this.props.className : null} 
				to={to}>
					{this.props.children}
			</Link>
		);
	}
}