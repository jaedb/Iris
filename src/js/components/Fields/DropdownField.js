import React from 'react';
import Icon from '../Icon';
import { removeDuplicates } from '../../util/arrays';

export default class DropdownField extends React.Component {
  constructor(props) {
    super(props);

    // Create a "unique" id. This is human-controlled to avoid requiring
    // other libraries for a very simple purpose: clicking outside
    this.uid = this.props.name.replace(' ', '_').toLowerCase();
    if (this.props.uid) {
      this.uid += `_${this.props.uid}`;
    }

    this.state = {
      expanded: false,
      changed: false,
    };

    this.handleClick = this.handleClick.bind(this);
  }

  componentDidMount() {
    window.addEventListener('click', this.handleClick, false);
  }

  componentWillUnmount() {
    window.removeEventListener('click', this.handleClick, false);
  }

  setExpanded(expanded = !this.state.expanded) {
    if (expanded) {
      this.setState({ expanded, changed: false });
      window.addEventListener('click', this.handleClick, false);
    } else {
      this.setState({ expanded });
      window.removeEventListener('click', this.handleClick, false);
      if (this.props.onClose && this.state.changed) {
        this.props.onClose();
      }
    }
  }

  handleClick(e) {
    // TODO: remove dependency on jQuery and explore the performance of this functionality
    if ($(e.target).closest('.dropdown-field').attr('data-uid') != this.uid && this.state.expanded) {
      this.setExpanded(false);
    }
  }

  handleChange(value, is_selected) {
    const current_value = this.props.value;
    this.setState({ changed: true });

    if (this.isMultiSelect()) {
      if (value == 'select-all') {
        var new_value = [];
        for (let i = 0; i < this.props.options.length; i++) {
          new_value.push(this.props.options[i].value);
        }
      } else if (is_selected) {
        const index = current_value.indexOf(value);
        current_value.splice(index, 1);
        var new_value = current_value;
      } else {
        current_value.push(value);
        var new_value = current_value;
      }
      new_value = removeDuplicates(new_value);
    } else {
      var new_value = value;

      // Collapse our menu
      this.setExpanded(false);
    }

    return this.props.handleChange(new_value);
  }

  isMultiSelect() {
    return this.props.value instanceof Array;
  }

  selectedOptions() {
    const {
      options: optionsProp,
      value,
    } = this.props;
    let selectedOptions = [];

    if (optionsProp) {
      // Value not set, default to first option
      if (value === null || value === undefined) {
        selectedOptions = [optionsProp[0]];
      } else if (this.isMultiSelect()) {
        for (const multiSelectValue of value) {
          selectedOptions = [
            ...selectedOptions,
            ...optionsProp.filter((option) => option.value === multiSelectValue),
          ];
        }
      } else {
        selectedOptions = optionsProp.filter((option) => option.value === value);
      }
    }

    return selectedOptions;
  }

  render() {
    const {
      options: optionsProp,
      no_status_icon,
      no_label,
      button,
      className: classNameProp = '',
      name,
      value,
      icon,
      icon_type,
      selected_icon: selectedIconProp,
      valueAsLabel,
      noLabel,
    } = this.props;
    const { expanded } = this.state;

    if (!optionsProp) return null;

    const selectedOptions = this.selectedOptions();

    const options = Object.assign([], optionsProp);
    if (this.isMultiSelect()) {
      options.push({
        value: 'select-all',
        label: 'Select all',
        className: 'mid_grey-text',
      });
    }

    let className = `dropdown-field ${classNameProp}`;
    if (expanded) className += ' dropdown-field--expanded';
    if (no_status_icon) className += ' dropdown-field--no-status-icon';
    if (no_label) className += ' dropdown-field--no-label';
    if (button) className += ' dropdown-field--buttonify';

    let selected_icon = <Icon name="check" />;
    if (selectedIconProp) {
      selected_icon = <Icon name={selectedIconProp} />;
    }

    return (
      <div className={className} data-uid={this.uid}>
        <div className={`dropdown-field__label${button ? ` button ${button}` : ''}`} onClick={(e) => this.setExpanded()}>
          {icon ? <Icon name={icon} type={icon_type || 'material'} /> : null}
          {!noLabel && (
            <span className="text">
              <span className="dropdown-field__label__value">
                {valueAsLabel && selectedOptions.length === 1 ? selectedOptions[0].label : name}
              </span>
              <span className="dropdown-field__label__name">
                {name}
              </span>
              {this.isMultiSelect() ? ` (${selectedOptions.length})` : null}
            </span>
          )}
        </div>
        <div className="dropdown-field__options">
          <div className="dropdown-field__options__liner">
            {
							options.map((option) => {
							  const is_selected = selectedOptions.includes(option);
							  return (
  <div
    className={`dropdown-field__options__item ${option.className ? option.className : ''}`}
    key={option.value}
    onClick={(e) => this.handleChange(option.value, is_selected)}
  >
    {!no_status_icon && is_selected && selected_icon}
    {option.label}
  </div>
							  );
							})
						}
          </div>
        </div>
      </div>
    );
  }
}
