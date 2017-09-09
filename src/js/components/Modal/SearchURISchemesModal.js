
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class SearchURISchemesModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			schemes: []
		}
	}

	componentDidMount(){
		this.setState({schemes: Object.assign([],this.props.search_uri_schemes)})
	}

	handleSubmit(e){
		this.props.uiActions.set({search_uri_schemes: this.state.schemes})
		this.props.uiActions.closeModal()
	}

	handleToggle(scheme){
		var uri_schemes = this.state.schemes
		var index = uri_schemes.indexOf(scheme)

		if (index > -1){
			uri_schemes.splice(index,1)
		} else {
			uri_schemes.push(scheme)
		}

		this.setState(uri_schemes)
	}

	render(){
		return (
			<div>
				<h1 className="no-bottom-padding">Search sources</h1>
				<h2 className="grey-text bottom-padding">Customise the providers used when searching. Reduce the number of providers to speed up your searches. Please note that not all backends support searching by artist, album or playlist.</h2>

				<form onSubmit={e => this.handleSubmit(e)}>
					<div className="list small">
						{
							this.props.available_uri_schemes.map(scheme => {
								return (								
									<div className="list-item field checkbox white" key={scheme}>
										<label>
											<input 
												type="checkbox"
												name={scheme}
												checked={this.state.schemes.indexOf(scheme) > -1}
												onChange={ e => this.handleToggle(scheme)} />
											<span className="label">{scheme.replace(':','')} &nbsp;<FontAwesome className="grey-text" name={helpers.sourceIcon(scheme)} /></span>
										</label>
									</div>
								)
							})
						}
						</div>

					<div className="actions centered-text">
						<button type="submit" className="primary wide">Save</button>
					</div>
				</form>
			</div>
		)
	}
}