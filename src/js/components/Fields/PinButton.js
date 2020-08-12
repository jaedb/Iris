import React from 'react';
import { useSelector, useDispatch } from 'react-redux'
import * as coreActions from '../../services/core/actions';
import Icon from '../Icon';

const PinButton = ({
  uri,
  className,
}) => {
  if (!uri) return null;

  const {
    removePinned,
    addPinned,
  } = coreActions;

  const dispatch = useDispatch();
  const isPinned = useSelector((state) => state.core.pinned ? state.core.pinned.find((item) => item === uri) : false)
  const remove = () => dispatch(removePinned(uri));
  const add = () => dispatch(addPinned(uri));

  if (isPinned) {
    return (
      <button
        type="button"
        className={`${className} button button--icon button--default`}
        onClick={remove}
      >
        <Icon name="star" />
      </button>
    );
  }
  return (
    <button
      type="button"
      className={`${className} button button--icon button--default`}
      onClick={add}
    >
      <Icon name="star_border" />
    </button>
  );
};

export default PinButton;
