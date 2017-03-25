
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

export default class DropdownField extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			expanded: false
		}
		this.handleClick = this.handleClick.bind(this)
	}

	componentDidMount(){		
		window.addEventListener("click", this.handleClick, false)
	}

	componentWillUnmount(){		
		window.removeEventListener("click", this.handleClick, false)
	}

	handleClick(e){
		// TODO: remove dependency on jQuery and explore the performance of this functionality
		if( $(e.target).closest('.dropdown-field').data('key') != this.props.name.replace(' ','_').toLowerCase() && this.state.expanded ){
			this.setState({ expanded: false })
		}
	}

	handleChange(value){
		this.setState({ expanded: !this.state.expanded })
		return this.props.handleChange( value )
	}

	handleToggle(){
		this.setState({ expanded: !this.state.expanded })
	}

	render(){
		if (!this.props.options) return null

		var classname = 'dropdown-field'
		if (this.state.expanded) classname += ' expanded'

		var current_value = this.props.options[0].value
		if (this.props.value) current_value = this.props.value

		var icon = <FontAwesome name="check" />
		if (typeof(this.props.reverse) !== 'undefined'){
			if (this.props.reverse){
				icon = <FontAwesome className="reverse" name="caret-down" />
			} else {
				icon = <FontAwesome className="reverse" name="caret-up" />
			}
		}

		return (
			<div className={classname} data-key={this.props.name.replace(' ','_').toLowerCase()}>
				<div className="label" onClick={ () => this.handleToggle() }>
					{this.props.icon ? <span><FontAwesome name={this.props.icon} />&nbsp; </span> : null}
					<span className="text">{ this.props.name }</span>
				</div>
				<div className="options">
					{
						this.props.options.map( option => {
							return (
								<div className="option" key={ option.value } onClick={ e => this.handleChange(option.value) }>
									{option.value == current_value ? icon : null}
									{option.label}
								</div>
							)
						})
					}
				</div>
			</div>
		)
	}
}