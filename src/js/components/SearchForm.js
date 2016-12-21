
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { createStore, bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as uiActions from '../services/ui/actions'
import * as spotifyActions from '../services/spotify/actions'

class SearchForm extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			query: ''
		}
	}

	handleSubmit(e){
		e.preventDefault();
		hashHistory.push(global.baseURL+'search/'+this.state.query);
		return false
	}

	render(){
		return (
			<form className={this.props.context+' search-form'} onSubmit={ e => this.handleSubmit(e) }>
				<input					
					type="text"
					placeholder="Search"
					onChange={ e => this.setState({ query: e.target.value }) } 
					value={ this.state.query } />
			</form>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(SearchForm)