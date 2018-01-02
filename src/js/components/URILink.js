
import React from 'react'
import { Link } from 'react-router'

export default class URILink extends React.Component{

	constructor(props){
		super(props);
	}

	handleContextMenu(e){
		if (this.props.onContextMenu){
			this.props.onContextMenu(e);
		}
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
				to = null;
		}

		return (
			<Link 
				className={this.props.className ? this.props.className : null} 
				to={to}
				onContextMenu={e => this.handleContextMenu(e)}>
					{this.props.children}
			</Link>
		);
	}
}