
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Player from '../components/Player'
import Icon from '../components/Icon'
import Thumbnail from '../components/Thumbnail'

import FontAwesome from 'react-fontawesome'
import * as mopidyActions from '../services/mopidy/actions'

class Sidebar extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		console.log( this.props )
	}

	render(){
		return (
			<aside>
				
				{ this.props.spotify && this.props.spotify.track ? <Thumbnail size="large" images={this.props.spotify.track.album.images} /> : null }

	        	<nav>

	        		<section>
						<Link activeClassName="active" to="/queue">
							<Icon name="play" className="white" />
							Now playing
						</Link>
					</section>

					<section>
						<title>Discover</title>
						<Link activeClassName="active" to="/discover">
							<Icon name="compass" className="white" />
							Discover
						</Link>
						<Link activeClassName="active" to="/discover/categories">
							<Icon name="grid" className="white" />
							Genre / Mood
						</Link>
						<Link activeClassName="active" to="/discover/featured">
							<Icon name="star" className="white" />
							Featured playlists
						</Link>
						<Link activeClassName="active" to="/discover/new-releases">
							<Icon name="leaf" className="white" />
							New releases
						</Link>
					</section>

					<section>
						<title>My Music</title>
						<Link activeClassName="active" to="/library/playlists">
							<Icon name="playlist" className="white" />
							Playlists
						</Link>
						<Link activeClassName="active" to="/library/artists">
							<Icon name="mic" className="white" />
							Artists
						</Link>
						<Link activeClassName="active" to="/library/albums">
							<Icon name="cd" className="white" />
							Albums
						</Link>
						<Link activeClassName="active" to="/library/tracks">
							<Icon name="music" className="white" />
							Tracks
						</Link>
					</section>

					<section>
						<Link activeClassName="active" to="/settings">
							<Icon name="cog" className="white" />
							Settings
						</Link>
					</section>

		        </nav>

		        <Player mini={ true } />

			</aside>
		);
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Sidebar)