
import React from 'react';
import Icon from '../Icon';

export default class TextField extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      in_focus: false,
      value: props.value || '',
      saved: false,
    };
  }

  static getDerivedStateFromProps({ value }, state) {
    if (!state.in_focus && state.value !== value) {
      return {
        value,
      };
    }
    return null;
  }

  handleChange = (e) => {
    this.setState({ value: e.target.value });
  }

  handleFocus = () => {
    this.setState({ in_focus: true, saved: false, });
  }

  handleBlur = () => {
    const { value, onChange, autosave } = this.props;
    const { value: stateValue } = this.state;
    this.setState({ in_focus: false });
    if (stateValue !== value) {
      onChange(stateValue);
      if (autosave) this.setState({ saved: true });
    }
  }

  render = () => {
    const {
      className,
      type = 'text',
      placeholder,
    } = this.props;
    const { value, saved } = this.state;

    return (
      <div className={`text-field__wrapper ${className}`}>
        <input
          type={type}
          onChange={this.handleChange}
          onFocus={this.handleFocus}
          onBlur={this.handleBlur}
          value={value}
          placeholder={placeholder}
        />
        {saved && <Icon name="check" className="text-field__saved" />}
      </div>
    );
  }
}
