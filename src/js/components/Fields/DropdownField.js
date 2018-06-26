
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Icon from '../Icon';

export default class DropdownField extends React.Component{

	constructor(props){
		super(props);

		// Create a "unique" id. This is human-controlled to avoid requiring
		// other libraries for a very simple purpose: clicking outside
		this.uid = this.props.name.replace(' ','_').toLowerCase();
		if (this.props.uid){
			this.uid += "_"+this.props.uid;
		}

		this.state = {
			expanded: false
		}

		this.handleClick = this.handleClick.bind(this);
	}

	componentDidMount(){		
		window.addEventListener("click", this.handleClick, false);
	}

	componentWillUnmount(){		
		window.removeEventListener("click", this.handleClick, false);
	}

	setExpanded(expanded = !this.state.expanded){
		if (expanded){
			this.setState({expanded: expanded});
			window.addEventListener("click", this.handleClick, false);
		} else {
			this.setState({expanded: expanded});
			window.removeEventListener("click", this.handleClick, false);
		}
	}

	handleClick(e){
		// TODO: remove dependency on jQuery and explore the performance of this functionality
		if ($(e.target).closest('.dropdown-field').attr('data-uid') != this.uid && this.state.expanded){
			this.setExpanded(false);
		}
	}

	handleChange(value, is_selected){

		var current_value = this.props.value;
		if (this.isMultiSelect()){
			if (value == 'select-all'){
				var new_value = [];
				for (var i = 0; i < this.props.options.length; i++){
					new_value.push(this.props.options[i].value);
				}
			} else if (is_selected){
				var index = current_value.indexOf(value);
				current_value.splice(index, 1);
				var new_value = current_value;
			} else {
				current_value.push(value);
				var new_value = current_value;
			}
		} else {
			var new_value = value;

			// Collapse our menu
			this.setExpanded(false);
		}

		return this.props.handleChange(new_value);
	}

	isMultiSelect(){
		return this.props.value instanceof Array;
	}

	render(){
		if (!this.props.options){
			return null;
		}

		var className = 'dropdown-field';
		if (this.state.expanded){
			className += ' expanded';
		}
		if (this.props.no_status_icon){
			className += ' no-status-icon';
		}
		if (this.props.no_label){
			className += ' no-label';
		}
		if (this.props.button){
			className += ' buttonify';
		}
		if (this.props.className){
			className += ' '+this.props.className;
		}
		var current_value = null;
		if (this.props.value !== undefined){
			current_value = this.props.value;
		} else if (this.props.options.length > 0){
			current_value = this.props.options[0].value;
		}

		var selected_icon = <Icon name="check" />
		if (this.props.selected_icon){
			selected_icon = <Icon name={this.props.selected_icon} />
		}

		var options = Object.assign([], this.props.options);
		if (this.isMultiSelect()){
			options.push({
				value: 'select-all',
				label: 'Select all',
				className: 'grey-text'
			});
		}

		return (
			<div className={className} data-uid={this.uid}>
				<div className={"label"+(this.props.button ? " button "+this.props.button : "")} onClick={e => this.setExpanded()}>
					{this.props.icon ? <Icon name={this.props.icon} type={this.props.icon_type ? this.props.icon_type : "material"} /> : null}
					<span className="text">
						{this.props.name}
						{this.isMultiSelect() ? ' ('+current_value.length+')' : null}
					</span>
				</div>
				<div className="options">
					<div className="liner">
						{
							options.map(option => {
								if (this.isMultiSelect()){
									var is_selected = current_value.indexOf(option.value) > -1;
								} else {
									var is_selected = current_value === option.value;
								}
								return (
									<div className={"option "+(option.className ? option.className : '')} key={option.value} onClick={e => this.handleChange(option.value, is_selected)}>
										{!this.props.no_status_icon && is_selected ? selected_icon : null}
										{option.label}
									</div>
								)
							})
						}
					</div>
				</div>
			</div>
		)
	}
}