
import React, { PropTypes } from 'react'
import Sortable from 'react-sortablejs';
import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class SourcesPriority extends React.Component{

	constructor(props){
		super(props);
	}

	handleSort(order){
		this.props.uiActions.set({uri_schemes_priority: order});
	}

	render(){
		var className = "sources-priority-field";
		var ordered_schemes = [];
		var unordered_schemes = [];

		for (var i = 0; i < this.props.uri_schemes.length; i++){
			var index = this.props.uri_schemes_priority.indexOf(this.props.uri_schemes[i]);

			if (index > -1){
				ordered_schemes[index] = this.props.uri_schemes[i];
			} else {
				unordered_schemes.push(this.props.uri_schemes[i]);
			}
		}

		for (var i = 0; i < unordered_schemes.length; i++){
			ordered_schemes.push(unordered_schemes[i]);
		}

		return (
			<Sortable
				className={className}
				onChange={(order, sortable, e) => {
					this.handleSort(order)
				}}>
					{
						ordered_schemes.map(scheme => {
							var name = helpers.titleCase(scheme.replace(':','').replace('+',' '));

							return (
								<span className="source flag grey" key={scheme} data-id={scheme}>
									<Icon name="drag_indicator" />
									{name}
								</span>
							);	
						})
					}
			</Sortable>
		);
	}
}