
import React, { PropTypes } from 'react'

export default class Dater extends React.Component{

	constructor(props) {
		super(props);
	}

	calculate(){
		switch(this.props.type){

			case 'length':
				var time = new Date( this.props.data );
				var min = time.getMinutes();
				var sec = time.getSeconds();
				if( sec < 10 ) sec = '0'+sec;
				return min+':'+sec;
				break

			case 'date':
				// TODO: nice date formatting
				return this.props.data
				break

			default:
				return null
		}
	}

	render(){
		return <span className="dater">{ this.calculate() }</span>
	}
}