import React from 'react';

import Link from '../Link';
import Icon from '../Icon';
import { i18n } from '../../locale';

export default class FilterField extends React.Component {
  constructor(props) {
    super(props);
    this.state = {
      value: '',
      active: (!!props.slim_mode),
    };

    this.handleKeyUp = this.handleKeyUp.bind(this);
  }

  componentDidMount() {
    const { initialValue: value } = this.props;

    window.addEventListener('keyup', this.handleKeyUp, false);
    this.setState({ value });
  }

  componentWillUnmount() {
    window.removeEventListener('keyup', this.handleKeyUp, false);
  }

  handleKeyUp = (e) => {
    const { slim_mode } = this.props;

    if (e.keyCode === 27 && !slim_mode) {
      e.preventDefault();
      this.handleChange({ target: { value: '' } });
    }
  }

  handleSubmit = (e) => {
    const { onSubmit } = this.props;

    if (onSubmit) onSubmit(e);
    e.preventDefault();
    return false;
  }

  activate = () => {
    this.setState({ active: true });
  }

  deactivate = () => {
    const {
      handleChange: doHandleChange,
    } = this.props;

    this.setState({ active: false, value: '' });
    doHandleChange('');
  }

  handleChange = ({ target: { value } }) => {
    const {
      slim_mode,
      handleChange: doHandleChange,
    } = this.props;

    this.setState({
      value,
      active: (slim_mode ? true : (value !== '')),
    });
    doHandleChange(value);
  }

  handleBlur = () => {
    const { value } = this.state;
    const { slim_mode } = this.props;

    if (value === '' && !slim_mode) {
      this.setState({ active: false });
    }
  }

  render = () => {
    const { value, active } = this.state;

    return (
      <span className={`filter-field ${active ? 'active' : ''}`}>
        <form onSubmit={this.handleSubmit}>
          <input
            type="text"
            placeholder={i18n('fields.filter')}
            value={value}
            onFocus={this.activate}
            onBlur={this.handleBlur}
            onChange={this.handleChange}
          />
          <Icon
            name={active ? 'close' : 'search'}
            type="material"
            onClick={active ? this.deactivate : null}
          />
        </form>
      </span>
    );
  }
}
