
import React from 'react'

export default class TextField extends React.Component{

	constructor(props){
		super(props);

		this.state = {
			in_focus: false,
			value: (this.props.value ? this.props.value : '')
		}
	}

	componentWillReceiveProps(newProps){
		if (!this.state.in_focus){
			this.setState({value: newProps.value})
		}
	}

	handleChange(e){
		this.setState({value: e.target.value});
	}

	handleFocus(e){
		this.setState({in_focus: true});
	}

	handleBlur(e){
		this.setState({in_focus: false});
		if (this.state.value !== this.props.value){
			this.props.onChange(this.state.value);
		}
	}

	render(){
		return (
			<input
				className={this.props.className ? this.props.className : ''}
				type={this.props.type ? this.props.type : "text"}
				onChange={e => this.handleChange(e)}
				onFocus={e => this.handleFocus(e)}
				onBlur={e => this.handleBlur(e)}
				value={this.state.value}
			/>
		);
	}
}