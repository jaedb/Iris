
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'

import * as actions from '../services/spotify/actions'

class SpotifyAuthenticationFrame extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e){
		console.log('handling click')
		this.props.actions.authorize();
	}

	render(){
		var src = '//jamesbarnsley.co.nz/spotmop.php?action=frame';
		if( this.props.spotify.authorizing ){
			src = '//jamesbarnsley.co.nz/spotmop.php?action=authorize&app='+location.protocol+'//'+window.location.host;
		}
		return (
			<div>
				<button onClick={(e) => this.handleClick(e)}>Authorize</button>
				<iframe src={src}></iframe>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SpotifyAuthenticationFrame)