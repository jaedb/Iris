
import React from 'react';

export default class TextField extends React.Component {
  constructor(props) {
    super(props);

    this.state = {
      in_focus: false,
      value: props.value || '',
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
    this.setState({ in_focus: true });
  }

  handleBlur = () => {
    const { value, onChange } = this.props;
    const { value: stateValue } = this.state;
    this.setState({ in_focus: false });
    if (stateValue !== value) {
      onChange(stateValue);
    }
  }

  render = () => {
    const {
      className,
      type = 'text',
      placeholder,
    } = this.props;
    const { value } = this.state;

    return (
      <input
        className={className}
        type={type}
        onChange={this.handleChange}
        onFocus={this.handleFocus}
        onBlur={this.handleBlur}
        value={value}
        placeholder={placeholder}
      />
    );
  }
}
