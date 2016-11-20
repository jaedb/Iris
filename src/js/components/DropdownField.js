
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
	}

	handleChange(value){
		this.setState({ expanded: !this.state.expanded })
		return this.props.handleChange( value )
	}

	handleToggle(){
		this.setState({ expanded: !this.state.expanded })
	}

	render(){
		if( !this.props.options ) return null

		var classname = 'dropdown-field'
		if( this.state.expanded ) classname += ' expanded'

		return (
			<div className={classname}>
				<div className="label" onClick={ () => this.handleToggle() }>
					<FontAwesome name={this.props.icon} />
					<span className="text">&nbsp; { this.props.name }</span>
				</div>
				<div className="options">
					{
						this.props.options.map( option => {
							return (
								<div className="option" key={ option.value } onClick={ e => this.handleChange(option.value) }>
									{ option.value == this.props.value ? <FontAwesome name="check" /> : null }
									{ option.label }
								</div>
							)
						})
					}
				</div>
			</div>
		)
	}
}