
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link, hashHistory } from 'react-router'
import { createStore, bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class SearchForm extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			query: ''
		}
	}

	handleSubmit(e){
		e.preventDefault()

		// check for uri type matching
		switch (helpers.uriType(this.state.query)){

			case 'album':
				hashHistory.push(global.baseURL+'album/'+this.state.query)
				break

			case 'artist':
				hashHistory.push(global.baseURL+'artist/'+this.state.query)
				break

			case 'playlist':
				hashHistory.push(global.baseURL+'playlist/'+this.state.query)
				break

			default:
				hashHistory.push(global.baseURL+'search/iris:search:all:'+this.state.query)
				break
		}

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

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapDispatchToProps)(SearchForm)