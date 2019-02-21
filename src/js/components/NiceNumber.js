
import React from 'react'

export default class NiceNumber extends React.Component{

	constructor(props){
		super(props);
	}

	format(){
		var formatted = parseInt(this.props.value);

		// > 1 million
		if (formatted > 1000000){
			formatted = formatted / 1000000;
			formatted = Math.round( formatted * 10 ) / 10;
			formatted = formatted+'m';

		// > 1 thousand
		} else if (formatted > 1000){
			formatted = formatted / 1000;
			formatted = Math.round( formatted * 10 ) / 10;
			formatted = formatted+'k';

		} else {
			formatted = formatted.toLocaleString();
		}

		return formatted;
	}

	render(){
		return <span className="counter">{this.format()}</span>;
	}
}
