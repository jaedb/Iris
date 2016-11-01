
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Header from '../../components/Header'

export default class LibraryLocal extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return (
			<div className="view library-local-view">
				<Header icon="folder" title="Local" />
				<Link to="/library/local/directory/local:directory">Folders</Link>
				<Link to="/library/local/artists">Artists</Link>
			</div>
		);
	}
}