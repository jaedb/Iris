
import React from 'react';
import Icon from '../Icon';

export default class ColourField extends React.Component{

	constructor(props){
		super(props);
	}

	render(){

		var colours = [
			'',
			'mid_grey',
			'grey',
			'dark_grey',
			'black',
			'turquoise',
			'green',
			'blue',
			'light_blue',
			'yellow',
			'orange',
			'red'
		];

		return (
			<div className="colour-field">
				{
					colours.map(colour => {
						return (
							<div 
								key={colour}
								className={"colour-field__option "+(colour ? colour+"-background " : "")+(this.props.colour == colour ? "colour-field__option--selected" : "")}
								onClick={e => this.props.onChange(colour)}>
								{this.props.colour == colour ? <Icon name="check" className="colour-field__option__icon" /> : null}
							</div>
						);
					})
				}
			</div>
		);
	}
}