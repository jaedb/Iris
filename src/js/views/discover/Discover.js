
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import GridSlider from '../../components/GridSlider'
import ArtistSentence from '../../components/ArtistSentence'

import * as helpers from '../../helpers'
import * as spotifyActions from '../../services/spotify/actions'

class Discover extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.props.spotifyActions.getDiscover()
	}

	renderRecommendations(){
		if (!this.props.discover || this.props.discover.length <= 0) return null

		return (
			<section className="recommendations">
				{
					this.props.discover.map(
						(discover, index) => {
							return (
								<div className="grid-slider-wrapper" key={index}>
									<h4>Because you listened to <ArtistSentence artists={discover.seed.artists} /></h4>
									<GridSlider tracks={discover.tracks} />
								</div>
							)
						}
					)
				}
			</section>
		)
	}

	render(){
		return (
			<div className="view discover-view">
				<Header icon="compass" title="Discover" />
				{ this.renderRecommendations() }
			</div>
		)
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		authorized: state.spotify.authorized,
		discover: state.spotify.discover
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Discover)