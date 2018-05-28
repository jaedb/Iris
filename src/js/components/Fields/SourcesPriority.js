
import React, { PropTypes } from 'react'
import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class SourcesPriority extends React.Component{

	constructor(props){
		super(props)
	}

	render(){
		var className = "sources-priority-field";

		return (
			<div className={className}>
				{
					this.props.uri_schemes.map(scheme => {
						var name = helpers.titleCase(scheme.replace(':','').replace('+',' '));

						return (
							<span className="source flag grey" key={scheme}>
								{name}
							</span>
						);	
					})
				}
			</div>
		);
	}
}