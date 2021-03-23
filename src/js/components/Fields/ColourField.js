import React, { memo } from 'react';
import Icon from '../Icon';

export default memo((props) => {
  const colours = [
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
    'red',
  ];

  return (
    <div className="colour-field">
      {
				colours.map((colour) => {
				  let text_colour = 'white';

				  switch (colour) {
				    case 'yellow':
				    case 'white':
				    case 'light_blue':
				      text_colour = 'black';
				      break;
				  }

				  return (
  <div
    key={colour}
    className={`colour-field__option ${colour ? `${colour}-background ` : ''}${props.colour == colour ? 'colour-field__option--selected' : ''}`}
    onClick={(e) => props.onChange(colour)}
  >
    {props.colour == colour ? <Icon name="check" className={`colour-field__option__icon ${text_colour}-text`} /> : null}
  </div>
				  );
				})
			}
    </div>
  );
});
