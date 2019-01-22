
import React from 'react';
import Link from './Link';

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
		var uri = this.props.uri;
		if (!this.props.unencoded){
			uri = encodeURIComponent(uri);
		}

		switch (this.props.type){			
			case 'playlist':
				to = '/playlist/'+uri;
				break;
			
			case 'artist':
				to = '/artist/'+uri;
				break;

			case 'album':
				to = '/album/'+uri;
				break;

			case 'track':
				to = '/track/'+uri;
				break;

			case 'user':
				to = '/user/'+uri;
				break;

			case 'browse':
				to = '/library/browse/'+uri;
				break;

			case 'recommendations':
				to = '/discover/recommendations/'+uri;
				break;

			case 'search':
				var exploded = uri.split('%3A');
				to = '/search/'+exploded[1]+'/'+exploded[2];
				break;

			default:
				to = null;
		}

		if (uri){
			return (
				<Link 
					className={this.props.className ? this.props.className : null} 
					to={to}
					onContextMenu={e => this.handleContextMenu(e)}>
						{this.props.children}
				</Link>
			);
		} else {
			return (
				<span className={this.props.className ? this.props.className : null}>
					{this.props.children}
				</span>
			);
		}
	}
}