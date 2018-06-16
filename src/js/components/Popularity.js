
import React from 'react'
import Icon from './Icon'
import * as helpers from '../helpers'

export default class Popularity extends React.Component{

	constructor(props){
		super(props)
	}

	render(){
		if (this.props.popularity === undefined || this.props.popularity === null){
			return null;
		}

		return (
			<span className="popularity-diagram">
				<span className={"bar"+(this.props.popularity > 10 ? " filled" : "")}></span>
				<span className={"bar"+(this.props.popularity > 30 ? " filled" : "")}></span>
				<span className={"bar"+(this.props.popularity > 50 ? " filled" : "")}></span>
				<span className={"bar"+(this.props.popularity > 70 ? " filled" : "")}></span>
				<span className={"bar"+(this.props.popularity > 90 ? " filled" : "")}></span>
			</span>
		);
	}
}