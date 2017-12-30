
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import Sortable from 'react-sortablejs';

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class SearchURISchemesModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			schemes: [],
			available_schemes: this.props.available_uri_schemes
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
		var unselected_schemes = [];
		for (var i = 0; i < this.props.available_uri_schemes.length; i++){
			var scheme = this.props.available_uri_schemes[i];
			if (!this.state.schemes.includes(scheme)){
				unselected_schemes.push(scheme);
			}
		}

		return (
			<div>
				<h1 className="no-bottom-padding">Search sources</h1>
				<h2 className="grey-text bottom-padding">Customise the providers used when searching. Reduce the number of providers to speed up your searches.</h2>

				<form onSubmit={e => this.handleSubmit(e)}>
					<Sortable
						className="list"
						options={{
							handle: '.drag-handle'
						}}
						onChange={(order, sortable, e) => {
							this.setState({schemes: order})
						}}>
							{
								this.state.schemes.map(scheme => {
									return (
										<div className="list-item draggable" key={scheme} data-id={scheme}>
											<FontAwesome className="grey-text drag-handle" name="bars" />
											{scheme.replace(':','')}
											<button className="discrete remove-uri no-hover" onClick={e => this.handleToggle(scheme)}>
												<FontAwesome name="trash" />
											</button>
										</div>
									)
								})
							}
					</Sortable>

					<div className="available-schemes">
						<h4>Add more schemes</h4>
						{
							unselected_schemes.map(scheme => {
								return (
									<div 
										className="scheme flag large blue" 
										key={scheme}
										onClick={e => this.handleToggle(scheme)}>
											{scheme.replace(':','')}
											&nbsp;&nbsp;
											<FontAwesome name="plus" />
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