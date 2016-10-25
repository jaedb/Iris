
import React, { PropTypes } from 'react'
import * as helpers from '../helpers'

export default class Parallax extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			image: false
		}
	}

	componentDidMount(){
		this.setState({
			image: helpers.SizedImages( this.props.images )
		})
		this.update();
	}

	update(){
		// TODO: copy canvas rules in here from Spotmop
	}

	render(){
		return (
			<div className="parallax">
				{ this.state.image ? <img src={this.state.image.huge} /> : null }
			</div>
		);
	}
}