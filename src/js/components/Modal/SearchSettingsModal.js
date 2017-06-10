
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class SearchSettingsModal extends React.Component{

	constructor(props){
		super(props)

		this.state = {
			spotify: true,
			uri_schemes: []
		}
	}

	componentDidMount(){
		if (this.props.search_settings){
			this.setState(Object.assign({},this.props.search_settings))
		} else {
			this.setState({
				spotify: true,
				uri_schemes: Object.assign([],this.props.uri_schemes)
			})
		}
	}

	handleSubmit(e){
		this.props.uiActions.set({search_settings: this.state})
		this.props.uiActions.closeModal()
		this.props.uiActions.startSearch(this.props.data.type, this.props.data.query)
	}

	handleToggle(scheme){
		var uri_schemes = this.state.uri_schemes
		var index = uri_schemes.indexOf(scheme)

		if (index > -1){
			uri_schemes.splice(index,1)
		} else {
			uri_schemes.push(scheme)
		}

		this.setState({uri_schemes: uri_schemes})
	}

	render(){
		return (
			<div>
				<h4 className="no-bottom-padding">Search sourced</h4>
				<h3 className="grey-text bottom-padding">Customise the providers used when searching. Reduce the number of providers to speed up your searches. Please note that not all backends support searching by artist, album or playlist.</h3>

				<form onSubmit={e => this.handleSubmit(e)}>
					<div className="list small">
						<div className="list-item field checkbox white">
							<label>
								<input 
									type="checkbox"
									name="enabled"
									checked={ this.state.spotify }
									onChange={ e => this.setState({ spotify: !this.state.spotify })} />
								<span className="label">spotify <span className="grey-text">(http)</span></span>
							</label>
						</div>
						{
							this.props.uri_schemes.map(scheme => {
								return (								
									<div className="list-item field checkbox white" key={scheme}>
										<label>
											<input 
												type="checkbox"
												name={scheme}
												checked={this.state.uri_schemes.indexOf(scheme) > -1}
												onChange={ e => this.handleToggle(scheme)} />
											<span className="label">{scheme.replace(':','')} <span className="grey-text">(mopidy)</span></span>
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