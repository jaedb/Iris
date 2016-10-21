
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import { Link } from 'react-router'

import ListItem from './ListItem'
import ItemSource from './ItemSource'

export default class PlaylistListItem extends React.Component{

	constructor(props) {
		super(props);
	}

	renderOwner(){
		if( !this.props.item.owner ) return null;
		return (			
			<Link to={ '/user/'+this.props.item.owner.uri }>
				{ this.props.item.owner.id }
			</Link>
		);
	}

	render(){
		return (
			<ListItem extraClasses="playlist" link={ '/playlist/'+this.props.item.uri }>
				<span className="col name">
					{this.props.item.name}
				</span>
				<span className="col owner">
					{ this.renderOwner() }
				</span>
				<span className="col source">
					<ItemSource uri={this.props.item.uri} />
				</span>
			</ListItem>
		);
	}
}