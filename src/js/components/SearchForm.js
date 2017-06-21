
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
			query: '',
			in_focus: false
		}
	}

	componentDidMount(){
		if (this.props.query){
			this.setState({query: this.props.query})
		}
	}

	componentWillReceiveProps(newProps){
		if (newProps.query && newProps.query != this.state.query && newProps.query != this.props.query && !this.state.in_focus){
			console.log('changing',newProps.query,this.props.query)
			this.setState({query: newProps.query})
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
			<form className="search-form" onSubmit={e => this.handleSubmit(e)}>
				<label>
					<input					
						type="text"
						placeholder="Search..."
						onChange={ e => this.setState({ query: e.target.value }) }
						onFocus={e => this.setState({in_focus: true})}
						onBlur={e => this.setState({in_focus: false})}
						value={ this.state.query } />
					</label>
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