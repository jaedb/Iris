
import React, { PropTypes } from 'react'

import Link from '../Link';
import Icon from '../Icon';

export default class FilterField extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			value: '',
			active: (this.props.slim_mode ? true : false)
		}

		this.handleKeyUp = this.handleKeyUp.bind(this)
	}

	componentWillMount(){
		window.addEventListener("keyup", this.handleKeyUp, false);
		this.setState({value: this.props.initialValue});
	}

	componentWillUnmount(){
		window.removeEventListener("keyup", this.handleKeyUp, false);
	}

	handleKeyUp(e){
		if (e.keyCode == 27 && !this.props.slim_mode){
			e.preventDefault()

			this.setState({
				value: '',
				active: false
			})

			this.handleChange('')
		}
	}

	activate(){
		this.setState({active: true})
	}

	handleChange(value){
		this.setState({
			value: value,
			active: (this.props.slim_mode ? true : (value != ''))
		})
		this.props.handleChange(value)
	}

	handleBlur(){
		if (this.state.value == '' && !this.props.slim_mode){
			this.setState({active: false})
		}
	}

	render(){
		return (
			<span className={"filter-field "+(this.state.active ? 'active' : '')} onClick={e => this.activate()}>
				<form>
					<input
						type="text"
						placeholder="Filter"
						value={this.state.value}
						onFocus={e => this.activate()}
						onBlur={e => this.handleBlur()}
						onChange={e => this.handleChange(e.target.value)}
					/>
					<Icon name="search" type="material" />
				</form>
			</span>
		)
	}
}