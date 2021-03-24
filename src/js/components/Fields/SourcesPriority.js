import React from 'react';
import Sortable from 'react-sortablejs';
import Icon from '../Icon';
import { titleCase } from '../../util/helpers';

export default class SourcesPriority extends React.Component {
  handleSort(order) {
    this.props.uiActions.set({ uri_schemes_priority: order });
  }

  render() {
    const className = 'sources-priority-field';
    const ordered_schemes = [];
    const unordered_schemes = [];

    for (var i = 0; i < this.props.uri_schemes.length; i++) {
      const index = this.props.uri_schemes_priority.indexOf(this.props.uri_schemes[i]);

      if (index > -1) {
        ordered_schemes[index] = this.props.uri_schemes[i];
      } else {
        unordered_schemes.push(this.props.uri_schemes[i]);
      }
    }

    for (var i = 0; i < unordered_schemes.length; i++) {
      ordered_schemes.push(unordered_schemes[i]);
    }

    return (
      <Sortable
        options={{
				  animation: 150,
        }}
        className={className}
        onChange={(order, sortable, e) => {
				  this.handleSort(order);
        }}
      >
        {
						ordered_schemes.map((scheme) => {
						  const name = titleCase(scheme.replace(':', '').replace('+', ' '));

						  return (
  <span className="source flag flag--grey" key={scheme} data-id={scheme}>
    <Icon name="drag_indicator" />
    {name}
  </span>
						  );
						})
					}
      </Sortable>
    );
  }
}
