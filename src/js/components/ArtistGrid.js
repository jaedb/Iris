
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Thumbnail from './Thumbnail'

import * as uiActions from '../services/ui/actions'

class ArtistGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = {
			e: e,
			context: 'artist',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	render(){
		if( this.props.artists ){
			var className = "grid artist-grid"
			if( this.props.className ) className += ' '+this.props.className
			return (
				<div className={className}>
					{
						this.props.artists.map(
							(artist, index) => {
								return (
									<Link 
										className="grid-item"
										to={global.baseURL+'artist/'+artist.uri}
										key={index} 
										onContextMenu={e => this.handleContextMenu(e,artist)}>
											<Thumbnail size="medium" images={artist.images} />
											<div className="name">{ artist.name }</div>
											<div className="secondary">
												{artist.followers ? artist.followers.total.toLocaleString()+' followers' : <span>0 followers</span>}
											</div>
									</Link>
								)
							}
						)
					}
				</div>
			);
		}
		return null;
	}
}

const mapStateToProps = (state, ownProps) => {
	return {}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ArtistGrid)

