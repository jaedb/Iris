
import React from 'react';
import Icon from '../Icon';

export default class ColourField extends React.Component{

	constructor(props){
		super(props);
	}

	render(){

		var colours = [
			'',
			'white',
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
						switch (colour){
							case 'yellow':
							case 'white':
							case 'light_blue':
								var text_colour = "black";
								break;
							default:
								var text_colour = "white";
								break;
						}
						return (
							<div 
								key={colour}
								className={"colour-field__option "+(colour ? colour+"-background " : "")+(this.props.colour == colour ? "colour-field__option--selected" : "")}
								onClick={e => this.props.onChange(colour)}>
								{this.props.colour == colour ? <Icon name="check" className={"colour-field__option__icon "+text_colour+"-text"} /> : null}
							</div>
						);
					})
				}
			</div>
		);
	}
}