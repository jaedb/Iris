import React from 'react';
import Icon from '../Icon';

const SelectField = ({ onChange, value, options }) => (
  <div className="select-field">
    <select
      onChange={(e) => onChange(e.target.value)}
      value={value}
    >
      {options.map((option) => (
        <option value={option.value} key={option.key || option.value}>
          {option.label}
        </option>
      ))}
    </select>
    <Icon name="arrow_drop_down" className="select-field__icon" />
  </div>
);

export default SelectField;
