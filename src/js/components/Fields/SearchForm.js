
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { hashHistory } from 'react-router'
import { createStore, bindActionCreators } from 'redux'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'

class SearchForm extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			query: '',
			in_focus: false
		}
	}

	componentDidMount(){
		if (this.props.query){
			var query = this.props.query.replace("search:","");
			this.setState({query: query});
		}
	}

	componentWillReceiveProps(newProps){
		if (newProps.query && newProps.query != this.state.query && !this.state.in_focus){
			this.setState({query: newProps.query.replace("search:","")});
		}
	}

	handleSubmit(e){
		e.preventDefault()

		// check for uri type matching
		switch (helpers.uriType(this.state.query)){

			case 'album':
				hashHistory.push(global.baseURL+'album/'+encodeURIComponent(this.state.query))
				break

			case 'artist':
				hashHistory.push(global.baseURL+'artist/'+encodeURIComponent(this.state.query))
				break

			case 'playlist':
				hashHistory.push(global.baseURL+'playlist/'+encodeURIComponent(this.state.query))
				break

			case 'track':
				hashHistory.push(global.baseURL+'track/'+encodeURIComponent(this.state.query))
				break

			default:
				var available_views = ["all:","artist:","album:","playlist:",":track"];
				var view_defined = false;
				var query = this.state.query;

				for (var i = 0; i < available_views.length; i++){
					if (query.startsWith(available_views[i])){
						view_defined = true;
					}
				}

				if (!view_defined){
					query = "all:"+query;
				}

				hashHistory.push(global.baseURL+'search/search:'+query);
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