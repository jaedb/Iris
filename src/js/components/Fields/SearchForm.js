
import React from 'react'
import { connect } from 'react-redux'

import { createStore, bindActionCreators } from 'redux'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'

class SearchForm extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			term: this.props.term
		}
	}

	handleSubmit(e){
		e.preventDefault();

		// check for uri type matching
		switch (helpers.uriType(this.state.term)){

			case 'album':
				this.props.history.push('/album/'+encodeURIComponent(this.state.term))
				break

			case 'artist':
				this.props.history.push('/artist/'+encodeURIComponent(this.state.term))
				break

			case 'playlist':
				this.props.history.push('/playlist/'+encodeURIComponent(this.state.term))
				break

			case 'track':
				this.props.history.push('/track/'+encodeURIComponent(this.state.term))
				break

			default:
				this.props.onSubmit(this.state.term);
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
						placeholder={this.props.term ? this.props.term : "Search..."}
						onChange={e => this.setState({term: e.target.value})}
						onBlur={e => (this.props.onBlur !== undefined ? this.props.onBlur(this.state.term) : null)}
						value={ this.state.term }
					/>
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