
import React, { PropTypes } from 'react'
import * as actions from './actions'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

class LibraryAlbums extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			albums: [] 
		};
	}

	// on load of this component
	componentDidMount(){

        var url = 'http://ws.audioscrobbler.com/2.0/?method=album.getinfo&api_key=4320a3ef51c9b3d69de552ac083c55e3&artist=Cher&album=Believe&format=json';
        //var url = 'https://pixabay.com/api/?key=3190651-5f431f6c829ace6d75ff07701&q=yellow+flowers&image_type=photo&pretty=true';

        var self = this;

        // url (required), options (optional)
		fetch(url, {
			method: 'get'
		}).then(function(response) {
			return response.json();
		}).then(function( json ) {
            self.setState({ albums: json.hits });
		}).catch(function(err) {
        	console.error( err );
		});
	}

	render() {
		return (
			<div>
				<h3>Library album</h3>
				{
					this.state.albums.map(album =>
						<h4 key={album.webformatURL}>{ album.webformatURL }</h4>
					)
				}
			</div>
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
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums)